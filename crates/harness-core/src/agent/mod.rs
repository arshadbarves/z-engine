//! The agent loop: turn orchestration, streaming consumption, permission
//! gating, tool execution, and cancellation (spec Â§4.2).
//!
//! Ownership model:
//! - one background tokio task owns the conversation and the loop;
//! - the UI world talks to it through [`AgentHandle`] (`Command`s in) and
//!   an [`EventRx`] (`Event`s out) â the TUI never touches tools/provider;
//! - aborts are cooperative: an atomic flag checked by the provider stream
//!   and every tool, plus `select!` points on the command channel between
//!   chunks and while awaiting approvals.

pub mod events;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::context::{
    self,
    budget::{BudgetMeter, Pressure},
    compact,
    notes::{NotesInput, NotesStore},
};
use crate::perms::{Decision, PolicyEngine};
use crate::provider::{
    AccumulatedToolCall, ChatMessage, ChatRequest, Client, ProviderError, StreamEvent, ToolCall,
    ToolCallAccumulator, Usage,
};
use crate::session::{SessionEvent, SessionWriter};
use crate::tools::{ToolCtx, ToolError, ToolOutput, ToolRegistry};

pub use events::{ApprovalDecision, Command, Event};

/// Safety valve against genuinely runaway loops. Spec says "no hard turn
/// cap"; 500 consecutive tool rounds is far beyond any real task and only
/// guards pathological provider behavior. Recorded in docs/deviations.md.
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 500;
const INPUT_PREVIEW_CHARS: usize = 160;

/// Everything the loop needs; built once at startup (headless or TUI).
#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub model: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub project_root: PathBuf,
    pub tmp_dir: PathBuf,
    /// Seed bash-prefix allow rules (from config files).
    pub initial_allow_rules: Vec<String>,
    /// Context budget (spec §6); drives warnings + auto-compaction.
    pub max_context_tokens: u32,
    /// Verbatim L2 tail size for compaction.
    pub keep_recent_messages: usize,
}

impl LoopConfig {
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: base_url.into(),
            api_key: None,
            project_root: PathBuf::from("."),
            tmp_dir: std::env::temp_dir(),
            initial_allow_rules: Vec::new(),
            max_context_tokens: 120_000,
            keep_recent_messages: compact::DEFAULT_KEEP_RECENT,
        }
    }
}

/// Cloneable sender-side handle for driving the agent.
#[derive(Debug, Clone)]
pub struct AgentHandle {
    cmd_tx: UnboundedSender<Command>,
}

impl AgentHandle {
    pub fn submit(&self, text: impl Into<String>) {
        let _ = self.cmd_tx.send(Command::SubmitMessage(text.into()));
    }

    pub fn approve(&self, id: u64, decision: crate::agent::events::ApprovalDecision) {
        let _ = self.cmd_tx.send(Command::Approve { id, decision });
    }

    pub fn deny(&self, id: u64) {
        let _ = self.cmd_tx.send(Command::Deny { id });
    }

    pub fn abort(&self) {
        let _ = self.cmd_tx.send(Command::Abort);
    }

    /// Force context compaction now (`/compact`).
    pub fn compact(&self) {
        let _ = self.cmd_tx.send(Command::Compact);
    }

    /// Dump the current L1 notes (`/notes`).
    pub fn request_notes(&self) {
        let _ = self.cmd_tx.send(Command::RequestNotes);
    }

    /// Ask the loop task to finish gracefully.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(Command::Shutdown);
    }
}

/// Receiver end of the core→UI event feed.
#[derive(Debug)]
pub struct EventRx {
    rx: UnboundedReceiver<Event>,
}

impl EventRx {
    pub async fn recv(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    pub fn try_recv(&mut self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

/// Spawn the agent task. Returns a command handle and the event feed.
pub fn spawn(cfg: LoopConfig) -> (AgentHandle, EventRx) {
    spawn_with_recorder(cfg, None, None)
}

/// Preloaded conversation state for `--resume`.
#[derive(Debug, Default)]
pub struct ResumeState {
    pub working: Vec<ChatMessage>,
    /// Raw note payloads: either `update_context_notes` argument objects or
    /// plain compaction-summary lines.
    pub note_payloads: Vec<String>,
}

/// Spawn with an optional session recorder (persistence) and optional
/// replayed state (resume).
pub fn spawn_with_recorder(
    cfg: LoopConfig,
    resume: Option<ResumeState>,
    recorder: Option<SessionWriter>,
) -> (AgentHandle, EventRx) {
    let abort_flag = Arc::new(AtomicBool::new(false));

    // Sub-agent runner shares the provider client and the parent's abort
    // flag so Esc tears down the whole subtree.
    let sub_client = match Client::new(&cfg.base_url, cfg.api_key.clone()) {
        Ok(c) => c,
        Err(e) => {
            let (_cmd_tx, _cmd_rx) = mpsc::unbounded_channel::<Command>();
            let (ev_tx, ev_rx) = mpsc::unbounded_channel::<Event>();
            tokio::spawn(async move {
                let _ = ev_tx.send(Event::Error(format!("provider init failed: {e}")));
            });
            return (AgentHandle { cmd_tx: _cmd_tx }, EventRx { rx: ev_rx });
        }
    };
    let model = cfg.model.clone();
    let project_root = cfg.project_root.clone();
    let tmp_dir = cfg.tmp_dir.clone();
    let sub_abort = Arc::clone(&abort_flag);
    let runner: crate::tools::SubAgentRunner = Arc::new(move |prompt: String, max_rounds: u32| {
        let client = sub_client.clone();
        let model = model.clone();
        let root = project_root.clone();
        let tmp = tmp_dir.clone();
        let abort = Arc::clone(&sub_abort);
        Box::pin(
            async move { run_isolated(client, model, root, tmp, abort, &prompt, max_rounds).await },
        )
    });

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (ev_tx, ev_rx) = mpsc::unbounded_channel::<Event>();

    match Client::new(&cfg.base_url, cfg.api_key.clone()) {
        Ok(client) => {
            let perms = Arc::new(Mutex::new(PolicyEngine::new(
                cfg.initial_allow_rules.clone(),
            )));
            let registry = ToolRegistry::builtins();
            tokio::spawn(agent_task(
                cfg, client, perms, registry, cmd_rx, ev_tx, resume, recorder, runner, abort_flag,
            ));
        }
        Err(e) => {
            // Surface asynchronously so callers can still attach to events.
            let ev_tx2 = ev_tx.clone();
            tokio::spawn(async move {
                let _ = ev_tx2.send(Event::Error(format!("provider init failed: {e}")));
            });
        }
    }
    (AgentHandle { cmd_tx }, EventRx { rx: ev_rx })
}

struct LoopState {
    /// Everything between the L0/L1 prefix and the current turn.
    working: Vec<ChatMessage>,
    approval_counter: u64,
    /// Last provider-reported usage (authoritative pressure signal).
    last_usage: Usage,
    /// Set by Command::Compact.
    force_compact: bool,
    /// Rendered repository symbol map, regenerated when dirty.
    repo_map_text: Option<String>,
}

impl LoopState {
    fn estimate_working(&self) -> u64 {
        let mut bytes = 0usize;
        for m in &self.working {
            let text = match m {
                ChatMessage::System { content }
                | ChatMessage::User { content }
                | ChatMessage::Tool { content, .. } => content.as_str(),
                ChatMessage::Assistant { content, .. } => content.as_deref().unwrap_or(""),
            };
            bytes += text.len();
        }
        // ~4 bytes per token for code/English; estimator calibrated in v1.0.
        (bytes as u64 / 4).max(if bytes > 0 { 1 } else { 0 })
    }

    fn pressure_tokens(&self) -> u64 {
        // Provider-reported usage is authoritative once available; before
        // that, fall back to the local estimator.
        let reported = self.last_usage.prompt_tokens + self.last_usage.completion_tokens;
        if reported > 0 {
            reported
        } else {
            self.estimate_working()
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn agent_task(
    cfg: LoopConfig,
    client: Client,
    perms: Arc<Mutex<PolicyEngine>>,
    registry: ToolRegistry,
    mut cmd_rx: UnboundedReceiver<Command>,
    ev_tx: UnboundedSender<Event>,
    resume: Option<ResumeState>,
    mut recorder: Option<SessionWriter>,
    runner: crate::tools::SubAgentRunner,
    abort_flag: Arc<AtomicBool>,
) {
    let notes = Arc::new(Mutex::new(NotesStore::default()));
    let mut ctx = ToolCtx::new(
        cfg.project_root.clone(),
        Arc::clone(&perms),
        cfg.tmp_dir.clone(),
    )
    .with_task_runner(runner);
    ctx.notes = Arc::clone(&notes);
    // Language server (spec section 9 v0.8): Rust projects with
    // rust-analyzer installed get compiler-grade tooling + edit hooks.
    if let Some(server) = crate::lsp::LspClient::probe(&cfg.project_root) {
        ctx.lsp = Some(Arc::new(crate::lsp::LspClient::new(
            &cfg.project_root,
            server,
        )));
        tracing::info!("rust-analyzer attached");
    }
    // L0 is rebuilt per-request by l0_message(); nothing to keep here.
    let meter = BudgetMeter::new(cfg.max_context_tokens);

    let mut state = LoopState {
        working: Vec::new(),
        approval_counter: 0,
        last_usage: Usage::default(),
        force_compact: false,
        repo_map_text: None,
    };
    // Seed from a previous session's transcript (resume).
    if let Some(rs) = resume {
        state.working = rs.working;
        if let Ok(mut n) = notes.lock() {
            for payload in rs.note_payloads {
                match serde_json::from_str::<NotesInput>(&payload) {
                    Ok(input) => {
                        n.merge(&input.progress, &input.decisions, &input.needs_later);
                        n.mark_droppable(&input.droppable);
                    }
                    Err(_) => n.add_summary(payload),
                }
            }
        }
    }

    while let Some(command) = next_action(&mut cmd_rx).await {
        match command {
            Command::SubmitMessage(user_text) => {
                if let Some(w) = recorder.as_mut() {
                    let _ = w.record(&SessionEvent::UserMsg {
                        text: user_text.clone(),
                    });
                }
                state.working.push(ChatMessage::user(user_text));
                let _ = ev_tx.send(Event::TurnStarted);

                let outcome = run_turn(
                    &cfg,
                    &client,
                    &registry,
                    &ctx,
                    &mut state,
                    &mut cmd_rx,
                    &ev_tx,
                    &abort_flag,
                    &meter,
                    &notes,
                    &mut recorder,
                )
                .await;

                match outcome {
                    TurnOutcome::Completed => {
                        let _ = ev_tx.send(Event::TurnCompleted {
                            prompt_tokens: state.last_usage.prompt_tokens,
                            completion_tokens: state.last_usage.completion_tokens,
                        });
                    }
                    TurnOutcome::Aborted => {
                        abort_flag.store(false, Ordering::Relaxed);
                        let _ = ev_tx.send(Event::TurnAborted);
                    }
                    TurnOutcome::Failed(msg) => {
                        let _ = ev_tx.send(Event::Error(msg));
                    }
                }
            }
            Command::Compact => {
                state.force_compact = true;
                let _ = ev_tx.send(Event::StatusNote("compaction requested".into()));
            }
            Command::RequestNotes => {
                let rendered = notes.lock().ok().and_then(|n| n.render_block());
                let _ = ev_tx.send(Event::StatusNote(
                    rendered.unwrap_or_else(|| "no context notes recorded".into()),
                ));
            }
            _ => { /* stale Approve/Deny/Abort while idle are ignored */ }
        }
    }
    tracing::debug!("agent task exiting");
}

/// L0 prefix message (system + AGENTS.md), rebuilt per request but
/// byte-stable across rounds unless AGENTS.md changes.
fn l0_message(cfg: &LoopConfig) -> ChatMessage {
    ChatMessage::system(context::build_system_prompt(
        &cfg.project_root,
        context::load_agents_md(&cfg.project_root).as_deref(),
    ))
}

/// Compaction driver (spec section 6): elide L4, summarize L3 into L1.
async fn compact_working_set(
    client: &Client,
    cfg: &LoopConfig,
    state: &mut LoopState,
    notes: &Arc<Mutex<NotesStore>>,
    ev_tx: &UnboundedSender<Event>,
    recorder: &mut Option<SessionWriter>,
) {
    let before = state.pressure_tokens();
    let mut outcome = compact::compact(&state.working, cfg.keep_recent_messages, &cfg.tmp_dir);

    if !outcome.summarize_input.is_empty() {
        let summary = summarize_segment(client, cfg, &outcome.summarize_input).await;
        if !summary.is_empty() {
            if let Some(w) = recorder.as_mut() {
                let _ = w.record(&SessionEvent::Note {
                    text: summary.clone(),
                });
            }
            if let Ok(mut n) = notes.lock() {
                n.add_summary(summary);
            }
        }
    }

    state.working = std::mem::take(&mut outcome.messages);
    let after = state.estimate_working();
    let _ = ev_tx.send(Event::StatusNote(format!(
        "context compacted: ~{} -> ~{} tokens ({} tool outputs elided)",
        before, after, outcome.elided_tool_outputs
    )));
}

/// Isolated sub-agent loop (spec section 9 v0.7): read-only toolset, own
/// transcript, bounded rounds; returns the final assistant text only.
async fn run_isolated(
    client: Client,
    model: String,
    project_root: PathBuf,
    tmp_dir: PathBuf,
    abort: Arc<AtomicBool>,
    prompt: &str,
    max_rounds: u32,
) -> Result<String, String> {
    const SUB_SYSTEM: &str = "You are a research sub-agent inside the harness coding agent.\nYou explore a repository read-only to answer one specific question.\nBe efficient: read only what is needed, then report.\nYour final message is delivered verbatim to the parent agent as the answer.\nStructure it as short factual bullet lines.";

    let perms = Arc::new(Mutex::new(PolicyEngine::new(Vec::new())));
    let mut ctx = ToolCtx::new(project_root.clone(), Arc::clone(&perms), tmp_dir);
    ctx.abort = Arc::clone(&abort);
    let registry = ToolRegistry::readonly_subset();

    let mut messages = vec![
        ChatMessage::system(SUB_SYSTEM),
        ChatMessage::user(prompt.to_string()),
    ];

    for round in 1..=max_rounds {
        if abort.load(Ordering::Relaxed) {
            return Err("aborted".into());
        }
        tracing::debug!(round, "sub-agent round");
        let request = ChatRequest::new(model.clone(), messages.clone()).with_tools(registry.defs());
        let mut stream = client.stream_chat(&request, Arc::clone(&abort));

        let mut text = String::new();
        let mut acc = ToolCallAccumulator::default();
        // No command watching: sub-agents die with the parent's flag.
        loop {
            tokio::select! {
                item = stream.recv() => match item {
                    None => break,
                    Some(Err(e)) => return Err(format!("provider error in sub-agent: {e}")),
                    Some(Ok(StreamEvent::TextDelta(t))) => text.push_str(&t),
                    Some(Ok(StreamEvent::ToolCallDelta { index, id, name, args_delta })) => {
                        acc.absorb(index, id.as_deref(), name.as_deref(), &args_delta);
                    }
                    Some(Ok(StreamEvent::Usage(_))) => {}
                    Some(Ok(StreamEvent::Finish(_))) => {}
                    Some(Ok(StreamEvent::Done)) => break,
                },
                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                    if abort.load(Ordering::Relaxed) {
                        return Err("aborted".into());
                    }
                }
            }
        }

        let finalized = acc.finish();
        eprintln!(
            "[DBG-SUB] round {round} text_len={} finalized={}",
            text.len(),
            finalized.len()
        );
        let mut complete_calls: Vec<ToolCall> = Vec::new();
        for call in finalized {
            match call {
                AccumulatedToolCall::Complete(c) => complete_calls.push(c),
                AccumulatedToolCall::MalformedArguments {
                    id,
                    name,
                    raw_arguments,
                    reason,
                } => {
                    let raw_short: String = raw_arguments.chars().take(160).collect();
                    messages.push(ChatMessage::tool_result(
                        id,
                        format!(
                            "ERROR: arguments not valid JSON ({reason}). You sent: {raw_short}"
                        ),
                    ));
                    let _ = name;
                }
                AccumulatedToolCall::MissingId { index } => {
                    messages.push(ChatMessage::user(format!(
                        "[harness] tool call index {index} had no id; skipped."
                    )));
                }
            }
        }

        messages.push(ChatMessage::Assistant {
            content: (!text.is_empty()).then_some(text),
            tool_calls: complete_calls.clone(),
        });

        eprintln!("[DBG-SUB] complete={} returning?", complete_calls.len());
        if complete_calls.is_empty() {
            return Ok(messages
                .last()
                .and_then(|m| match m {
                    ChatMessage::Assistant {
                        content: Some(c), ..
                    } => Some(c.clone()),
                    _ => None,
                })
                .unwrap_or_default());
        }

        for call in &complete_calls {
            if abort.load(Ordering::Relaxed) {
                return Err("aborted".into());
            }
            let input = parse_input(&call.function.arguments);
            let content = match registry.get(&call.function.name) {
                Some(tool) => match tool.run(input, &ctx).await {
                    Ok(out) => out.result,
                    Err(e) => format!("ERROR: {e}"),
                },
                None => format!("ERROR: unknown tool {}", call.function.name),
            };
            messages.push(ChatMessage::tool_result(call.id.clone(), content));
        }
    }

    Err(format!(
        "sub-agent hit its {max_rounds}-round limit without concluding"
    ))
}

/// Side-request that compresses demoted turns into terse summary bullets.
async fn summarize_segment(client: &Client, cfg: &LoopConfig, input: &str) -> String {
    const SUMMARIZER_SYSTEM: &str = "You compress an earlier portion of a coding-agent session.\nOutput terse markdown bullet lines under exactly three headings:\nFACTS / DECISIONS / OPEN THREADS. Keep file paths, names, numbers. No preamble.";

    let clipped: String = input.chars().take(12_000).collect();
    let req = ChatRequest::new(
        cfg.model.clone(),
        vec![
            ChatMessage::system(SUMMARIZER_SYSTEM),
            ChatMessage::user(clipped),
        ],
    );
    let abort = Arc::new(AtomicBool::new(false));
    let mut rx = client.stream_chat(&req, abort);
    let mut out = String::new();
    while let Some(item) = rx.recv().await {
        match item {
            Ok(StreamEvent::TextDelta(t)) => out.push_str(&t),
            Ok(StreamEvent::Done) | Ok(StreamEvent::Finish(_)) => {}
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "summarizer stream failed");
                return String::new();
            }
        }
    }
    out.trim().to_string()
}

/// Wait for a meaningful action; channel close / Shutdown ends the task.
async fn next_action(cmd_rx: &mut UnboundedReceiver<Command>) -> Option<Command> {
    match cmd_rx.recv().await {
        None | Some(Command::Shutdown) => None,
        Some(c) => Some(c),
    }
}

enum TurnOutcome {
    Completed,
    Aborted,
    Failed(String),
}

#[allow(clippy::too_many_arguments)]
async fn run_turn(
    cfg: &LoopConfig,
    client: &Client,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    state: &mut LoopState,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    abort_flag: &Arc<AtomicBool>,
    meter: &BudgetMeter,
    notes: &Arc<Mutex<NotesStore>>,
    recorder: &mut Option<SessionWriter>,
) -> TurnOutcome {
    let mut rounds: u32 = 0;

    loop {
        if abort_flag.load(Ordering::Relaxed) {
            return TurnOutcome::Aborted;
        }

        rounds += 1;
        if rounds > DEFAULT_MAX_TOOL_ROUNDS {
            return TurnOutcome::Failed(format!(
                "runaway-loop guard tripped after {DEFAULT_MAX_TOOL_ROUNDS} tool rounds"
            ));
        }

        // ---- pressure management (spec §6) ---------------------------
        if state.force_compact || meter.level(state.pressure_tokens()) == Pressure::Compact {
            compact_working_set(client, cfg, state, notes, ev_tx, recorder).await;
            state.force_compact = false;
        } else if meter.level(state.pressure_tokens()) == Pressure::Warn {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "context at {} tokens ({}% of budget)",
                state.pressure_tokens(),
                state.pressure_tokens() * 100 / u64::from(meter.max_tokens.max(1))
            )));
        }
        // Eager droppable elision — every round, pressure or not.
        {
            let ids = notes
                .lock()
                .map(|mut n| n.take_droppable_ids())
                .unwrap_or_default();
            let elided = compact::elide_droppable(&mut state.working, &ids, &cfg.tmp_dir);
            if elided > 0 {
                let _ = ev_tx.send(Event::StatusNote(format!(
                    "dropped {elided} marked tool output(s) from context"
                )));
            }
        }

        // ---- assemble L0 + repo map + L1 + working -------------------
        use std::sync::atomic::Ordering as AtomicOrdering;
        if ctx.repo_map_dirty.swap(false, AtomicOrdering::Relaxed) || state.repo_map_text.is_none()
        {
            state.repo_map_text = Some(context::repo_map::refresh_repo_map(ctx));
            tracing::debug!("repo map refreshed");
        }

        let mut request_messages = Vec::with_capacity(state.working.len() + 3);
        request_messages.push(l0_message(cfg));
        if let Some(map) = &state.repo_map_text {
            if !map.is_empty() {
                request_messages.push(ChatMessage::system(map.clone()));
            }
        }
        if let Some(notes_block) = notes.lock().ok().and_then(|n| n.render_block()) {
            request_messages.push(ChatMessage::system(notes_block));
        }
        request_messages.extend(state.working.iter().cloned());

        let request =
            ChatRequest::new(cfg.model.clone(), request_messages).with_tools(registry.defs());
        let mut stream = client.stream_chat(&request, Arc::clone(abort_flag));

        // ---- consume the stream --------------------------------------
        let mut text = String::new();
        let mut acc = ToolCallAccumulator::default();
        let outcome = consume_stream(
            &mut stream,
            cmd_rx,
            ev_tx,
            &mut text,
            &mut acc,
            &mut state.last_usage,
            abort_flag,
        )
        .await;

        match outcome {
            StreamOutcome::Aborted => return TurnOutcome::Aborted,
            StreamOutcome::Failed(e) => return TurnOutcome::Failed(e.to_string()),
            StreamOutcome::Completed => {}
        }

        // ---- assemble the assistant message --------------------------
        let finalized = acc.finish();
        let mut complete_calls: Vec<ToolCall> = Vec::new();
        let mut synthetic_errors: Vec<(String, String)> = Vec::new();

        for call in finalized {
            match call {
                AccumulatedToolCall::Complete(c) => complete_calls.push(c),
                AccumulatedToolCall::MalformedArguments {
                    id,
                    name,
                    raw_arguments,
                    reason,
                } => {
                    tracing::warn!(tool = ?name, %reason, "malformed tool arguments");
                    let raw_short: String = raw_arguments.chars().take(200).collect();
                    synthetic_errors.push((
                        id,
                        format!(
                            "ERROR: arguments were not valid JSON ({reason}). You sent: {raw_short}"
                        ),
                    ));
                }
                AccumulatedToolCall::MissingId { index } => {
                    tracing::warn!(index, "tool-call delta without id; skipped");
                    state.working.push(ChatMessage::user(format!(
                        "[harness] a tool call (index {index}) arrived without an id and was skipped."
                    )));
                }
            }
        }

        if let Some(w) = recorder.as_mut() {
            let _ = w.record(&SessionEvent::AssistantMsg {
                content: (!text.is_empty()).then(|| text.clone()),
                tool_calls: complete_calls
                    .iter()
                    .map(|c| crate::session::PersistedToolCall {
                        id: c.id.clone(),
                        name: c.function.name.clone(),
                        arguments: c.function.arguments.clone(),
                    })
                    .collect(),
            });
        }
        state.working.push(ChatMessage::Assistant {
            content: (!text.is_empty()).then_some(text),
            tool_calls: complete_calls.clone(),
        });
        for (id, content) in synthetic_errors {
            if let Some(w) = recorder.as_mut() {
                let _ = w.record(&SessionEvent::ToolResult {
                    tool_call_id: id.clone(),
                    content: content.clone(),
                });
            }
            state.working.push(ChatMessage::tool_result(id, content));
        }

        if complete_calls.is_empty() {
            return TurnOutcome::Completed;
        }
        // Even when finish_reason â  tool_calls, emitted calls demand execution.

        // ---- permissions + execution ---------------------------------
        match execute_calls(
            complete_calls,
            registry,
            ctx,
            cmd_rx,
            ev_tx,
            state,
            abort_flag,
        )
        .await
        {
            ExecutionsOutcome::Ran(results) => {
                for (call_id, content) in results {
                    if let Some(w) = recorder.as_mut() {
                        let _ = w.record(&SessionEvent::ToolResult {
                            tool_call_id: call_id.clone(),
                            content: content.clone(),
                        });
                    }
                    state
                        .working
                        .push(ChatMessage::tool_result(call_id, content));
                }
            }
            ExecutionsOutcome::Aborted => return TurnOutcome::Aborted,
        }
    }
}

enum StreamOutcome {
    Completed,
    Aborted,
    Failed(ProviderError),
}

/// Consume provider events, forwarding text deltas to the UI, until Done /
/// error / abort. Watches the command channel concurrently so Abort is
/// honored mid-stream.
async fn consume_stream(
    stream: &mut tokio::sync::mpsc::Receiver<Result<StreamEvent, ProviderError>>,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    text: &mut String,
    acc: &mut ToolCallAccumulator,
    usage_out: &mut Usage,
    abort_flag: &Arc<AtomicBool>,
) -> StreamOutcome {
    loop {
        tokio::select! {
            item = stream.recv() => {
                match item {
                    None => break,
                    Some(Err(e)) => return StreamOutcome::Failed(e),
                    Some(Ok(ev)) => match ev {
                        StreamEvent::TextDelta(t) => {
                            text.push_str(&t);
                            let _ = ev_tx.send(Event::TokenDelta(t));
                        }
                        StreamEvent::ToolCallDelta { index, id, name, args_delta } => {
                            acc.absorb(index, id.as_deref(), name.as_deref(), &args_delta);
                        }
                        StreamEvent::Usage(u) => {
                            // Latest prompt size + running completion total â
                            // the budget-pressure signal for v0.3's compactor.
                            usage_out.prompt_tokens = usage_out.prompt_tokens.max(u.prompt_tokens);
                            usage_out.completion_tokens =
                                usage_out.completion_tokens.saturating_add(u.completion_tokens);
                            let _ = ev_tx.send(Event::UsageUpdated {
                                prompt_tokens: usage_out.prompt_tokens,
                                completion_tokens: usage_out.completion_tokens,
                            });
                        }
                        // Non-terminal: usage may still arrive in later
                        // chunks (or already did in this batch).
                        StreamEvent::Finish(_) => {}
                        StreamEvent::Done => break,
                    },
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    None => {
                        abort_flag.store(true, Ordering::Relaxed);
                        return StreamOutcome::Aborted;
                    }
                    Some(Command::Abort) => {
                        abort_flag.store(true, Ordering::Relaxed);
                        return StreamOutcome::Aborted;
                    }
                    Some(_) => {} // approvals/submits are meaningless mid-stream
                }
            }
        }
    }
    StreamOutcome::Completed
}

enum ExecutionsOutcome {
    /// `(tool_call_id, transcript content)` in original call order.
    Ran(Vec<(String, String)>),
    Aborted,
}

enum Verdict {
    Run,
    Denied,
}

async fn execute_calls(
    calls: Vec<ToolCall>,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    state: &mut LoopState,
    abort_flag: &Arc<AtomicBool>,
) -> ExecutionsOutcome {
    // Phase 1 â decide every call up front (approvals surface sequentially).
    let mut verdicts: Vec<Verdict> = Vec::with_capacity(calls.len());
    for call in &calls {
        let input = parse_input(&call.function.arguments);
        let decision = ctx
            .perms
            .lock()
            .map(|p| p.decide(&call.function.name, &input))
            .unwrap_or(Decision::Gate);

        verdicts.push(match decision {
            Decision::Allow => Verdict::Run,
            Decision::Gate => {
                let suggested_rule = (call.function.name == "bash").then(|| {
                    PolicyEngine::suggested_rule(
                        input.get("command").and_then(|v| v.as_str()).unwrap_or(""),
                    )
                });
                // Outside the project root? Then "persist" is disabled
                // (spec section 5) and the call always gates on future runs.
                let target_outside = input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(|p| ctx.is_outside_root(Path::new(p)))
                    .unwrap_or(false);
                state.approval_counter += 1;
                let id = state.approval_counter;
                let detail = registry
                    .get(&call.function.name)
                    .and_then(|t| t.approval_preview(&input, ctx));
                let _ = ev_tx.send(Event::ApprovalRequired {
                    id,
                    tool: call.function.name.clone(),
                    input_preview: input_preview(&input),
                    suggested_rule: suggested_rule.clone(),
                    detail_preview: detail,
                    can_persist: !target_outside && call.function.name == "bash",
                });

                match wait_for_approval(id, cmd_rx, abort_flag).await {
                    ApprovalResolution::Granted(decision) => match decision {
                        crate::agent::events::ApprovalDecision::Once => Verdict::Run,
                        crate::agent::events::ApprovalDecision::AlwaysSession { rule } => {
                            if let Ok(mut p) = ctx.perms.lock() {
                                p.add_session_rule(rule);
                            }
                            Verdict::Run
                        }
                        crate::agent::events::ApprovalDecision::AlwaysPersist { rule } => {
                            match crate::config::persist_bash_rule(&ctx.project_root, &rule) {
                                Err(e) => {
                                    tracing::warn!(error = %e, "failed persisting rule");
                                    let _ = ev_tx.send(Event::StatusNote(format!(
                                        "could not persist rule: {e}"
                                    )));
                                }
                                Ok(_) => {
                                    let _ = ev_tx.send(Event::StatusNote(format!(
                                        "rule \"{rule}\" persisted to .harness/config.toml"
                                    )));
                                }
                            }
                            if let Ok(mut p) = ctx.perms.lock() {
                                p.add_session_rule(rule);
                            }
                            Verdict::Run
                        }
                    },
                    ApprovalResolution::Denied => Verdict::Denied,
                    ApprovalResolution::AbortTurn => return ExecutionsOutcome::Aborted,
                }
            }
        });
    }

    // Phase 2 â run: concurrency-safe tools together, unsafe ones serially.
    let mut outcomes: HashMap<usize, String> = HashMap::new();
    let mut safe_batch: Vec<(usize, ToolCall)> = Vec::new();
    for (idx, call) in calls.iter().enumerate() {
        if !matches!(verdicts[idx], Verdict::Run) {
            continue;
        }
        let safe = registry
            .get(&call.function.name)
            .map(|t| t.concurrency_safe())
            .unwrap_or(false);
        if safe {
            safe_batch.push((idx, call.clone()));
        }
    }

    if !safe_batch.is_empty() {
        let futs = safe_batch.iter().map(|(_, call)| {
            let call = call.clone();
            let ctx = ctx.clone();
            let ev_tx = ev_tx.clone();
            async move { run_one(call, &ctx, registry, &ev_tx).await }
        });
        let done = futures::future::join_all(futs).await;
        for ((idx, _), content) in safe_batch.iter().zip(done) {
            outcomes.insert(*idx, content);
        }
    }

    for (idx, call) in calls.iter().enumerate() {
        if outcomes.contains_key(&idx) || !matches!(verdicts[idx], Verdict::Run) {
            continue;
        }
        outcomes.insert(idx, run_one(call.clone(), ctx, registry, ev_tx).await);
    }

    // Phase 3 â transcript entries in original order; denials become polite
    // refusals addressed to the same tool_call_id (spec Â§5).
    let refusal = "The user declined permission for this action. Do not retry it \
                   unchanged; adjust your approach or explain what you need.";
    let ordered = calls
        .iter()
        .enumerate()
        .map(|(idx, call)| {
            let content = outcomes
                .get(&idx)
                .cloned()
                .unwrap_or_else(|| refusal.to_string());
            (call.id.clone(), content)
        })
        .collect();
    ExecutionsOutcome::Ran(ordered)
}

enum ApprovalResolution {
    Granted(crate::agent::events::ApprovalDecision),
    Denied,
    AbortTurn,
}

async fn wait_for_approval(
    id: u64,
    cmd_rx: &mut UnboundedReceiver<Command>,
    abort_flag: &Arc<AtomicBool>,
) -> ApprovalResolution {
    loop {
        match cmd_rx.recv().await {
            None => {
                abort_flag.store(true, Ordering::Relaxed);
                return ApprovalResolution::AbortTurn;
            }
            Some(Command::Approve { id: got, decision }) if got == id => {
                return ApprovalResolution::Granted(decision);
            }
            Some(Command::Deny { id: got }) if got == id => return ApprovalResolution::Denied,
            Some(Command::Abort) | Some(Command::Shutdown) => {
                abort_flag.store(true, Ordering::Relaxed);
                return ApprovalResolution::AbortTurn;
            }
            Some(_) => {} // mismatched ids / stray submits ignored
        }
    }
}

fn parse_input(arguments: &str) -> serde_json::Value {
    serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null)
}

/// Execute one allowed/approved call: events + timing + error mapping.
/// Errors become `"ERROR: â¦"` transcript text (self-correction path).
async fn run_one(
    call: ToolCall,
    ctx: &ToolCtx,
    registry: &ToolRegistry,
    ev_tx: &UnboundedSender<Event>,
) -> String {
    let started = Instant::now();
    let input = parse_input(&call.function.arguments);
    let preview = input_preview(&input);
    let name = call.function.name.clone();
    let _ = ev_tx.send(Event::ToolCallStarted {
        name: name.clone(),
        preview,
    });

    if ctx.aborted() {
        return "[aborted]".to_string();
    }
    let input_hook = input.clone();

    let result: Result<ToolOutput, ToolError> = match registry.get(&name) {
        Some(tool) => tool.run(input, ctx).await,
        None => Err(ToolError::Failed(format!("unknown tool: {name}"))),
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let mut out = match result {
        Ok(out) => out,
        Err(e) => {
            let _ = ev_tx.send(Event::ToolCallFinished {
                name,
                ok: false,
                duration_ms,
                summary: e.to_string(),
            });
            return format!("ERROR: {e}");
        }
    };

    // Diagnostics-after-edit hook: rust-analyzer feedback lands inside the
    // same tool-result so the model fixes errors immediately (spec 9 v0.8).
    crate::tools::lsp_tools::maybe_attach_diagnostics(
        &name,
        out.ok,
        &input_hook,
        ctx,
        &mut out.result,
    )
    .await;

    let _ = ev_tx.send(Event::ToolCallFinished {
        name,
        ok: out.ok,
        duration_ms,
        summary: out.summary,
    });
    out.result
}

fn input_preview(input: &serde_json::Value) -> String {
    let s = serde_json::to_string(input).unwrap_or_else(|_| "<unserializable>".into());
    let mut s: String = s.chars().take(INPUT_PREVIEW_CHARS).collect();
    if s.chars().count() == INPUT_PREVIEW_CHARS {
        s.push('\u{2026}');
    }
    s
}
