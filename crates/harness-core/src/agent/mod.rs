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
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::context;
use crate::perms::{Decision, PolicyEngine};
use crate::provider::{
    AccumulatedToolCall, ChatMessage, ChatRequest, Client, ProviderError,
    StreamEvent, ToolCall, ToolCallAccumulator, Usage,
};
use crate::tools::{ToolCtx, ToolError, ToolOutput, ToolRegistry};

pub use events::{Command, Event};

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

    pub fn approve(&self, id: u64, prefix_rule: Option<String>) {
        let _ = self.cmd_tx.send(Command::Approve { id, prefix_rule });
    }

    pub fn deny(&self, id: u64) {
        let _ = self.cmd_tx.send(Command::Deny { id });
    }

    pub fn abort(&self) {
        let _ = self.cmd_tx.send(Command::Abort);
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
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (ev_tx, ev_rx) = mpsc::unbounded_channel::<Event>();

    match Client::new(&cfg.base_url, cfg.api_key.clone()) {
        Err(e) => {
            // Surface asynchronously so callers can still attach to events.
            tokio::spawn(async move {
                let _ = ev_tx.send(Event::Error(format!("provider init failed: {e}")));
            });
            return (AgentHandle { cmd_tx }, EventRx { rx: ev_rx });
        }
        Ok(client) => {
            let perms = Arc::new(Mutex::new(PolicyEngine::new(cfg.initial_allow_rules.clone())));
            let registry = ToolRegistry::builtins_v01();
            tokio::spawn(agent_task(cfg, client, perms, registry, cmd_rx, ev_tx));
            (AgentHandle { cmd_tx }, EventRx { rx: ev_rx })
        }
    }
}

struct LoopState {
    messages: Vec<ChatMessage>,
    cumulative_usage: Usage,
    approval_counter: u64,
}

async fn agent_task(
    cfg: LoopConfig,
    client: Client,
    perms: Arc<Mutex<PolicyEngine>>,
    registry: ToolRegistry,
    mut cmd_rx: UnboundedReceiver<Command>,
    ev_tx: UnboundedSender<Event>,
) {
    let abort_flag = Arc::new(AtomicBool::new(false));
    let ctx = ToolCtx::new(cfg.project_root.clone(), Arc::clone(&perms), cfg.tmp_dir.clone());
    let system_prompt = context::build_system_prompt(
        &cfg.project_root,
        context::load_agents_md(&cfg.project_root).as_deref(),
    );

    let mut state = LoopState {
        messages: vec![ChatMessage::system(system_prompt)],
        cumulative_usage: Usage::default(),
        approval_counter: 0,
    };

    while let Some(command) = next_action(&mut cmd_rx).await {
        let Command::SubmitMessage(user_text) = command else {
            continue; // stale Approve/Deny/Abort while idle â ignore
        };

        state.messages.push(ChatMessage::user(user_text));
        let _ = ev_tx.send(Event::TurnStarted);

        let outcome = run_turn(
            &cfg, &client, &registry, &ctx, &mut state, &mut cmd_rx, &ev_tx, &abort_flag,
        )
        .await;

        match outcome {
            TurnOutcome::Completed => {
                let _ = ev_tx.send(Event::TurnCompleted {
                    prompt_tokens: state.cumulative_usage.prompt_tokens,
                    completion_tokens: state.cumulative_usage.completion_tokens,
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
    tracing::debug!("agent task exiting");
}

/// Wait for a meaningful action, transparently dropping stale ones.
async fn next_action(cmd_rx: &mut UnboundedReceiver<Command>) -> Option<Command> {
    loop {
        match cmd_rx.recv().await {
            None | Some(Command::Shutdown) => return None,
            Some(c) => return Some(c),
        }
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

        let request =
            ChatRequest::new(cfg.model.clone(), state.messages.clone()).with_tools(registry.defs());
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
            &mut state.cumulative_usage,
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
                    state.messages.push(ChatMessage::user(format!(
                        "[harness] a tool call (index {index}) arrived without an id and was skipped."
                    )));
                }
            }
        }

        state.messages.push(ChatMessage::Assistant {
            content: (!text.is_empty()).then_some(text),
            tool_calls: complete_calls.clone(),
        });
        for (id, content) in synthetic_errors {
            state.messages.push(ChatMessage::tool_result(id, content));
        }

        if complete_calls.is_empty() {
            return TurnOutcome::Completed;
        }
        // Even when finish_reason â  tool_calls, emitted calls demand execution.

        // ---- permissions + execution ---------------------------------
        match execute_calls(complete_calls, registry, ctx, cmd_rx, ev_tx, state, abort_flag).await {
            ExecutionsOutcome::Ran(results) => {
                for (call_id, content) in results {
                    state.messages.push(ChatMessage::tool_result(call_id, content));
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
                    PolicyEngine::suggested_rule(input.get("command").and_then(|v| v.as_str())
                        .unwrap_or(""))
                });
                state.approval_counter += 1;
                let id = state.approval_counter;
                let _ = ev_tx.send(Event::ApprovalRequired {
                    id,
                    tool: call.function.name.clone(),
                    input_preview: input_preview(&input),
                    suggested_rule,
                });

                match wait_for_approval(id, cmd_rx, abort_flag).await {
                    ApprovalResolution::Granted(prefix_rule) => {
                        if let Some(rule) = prefix_rule {
                            if let Ok(mut p) = ctx.perms.lock() {
                                p.add_session_rule(rule);
                            }
                        }
                        Verdict::Run
                    }
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
            let content = outcomes.get(&idx).cloned().unwrap_or_else(|| refusal.to_string());
            (call.id.clone(), content)
        })
        .collect();
    ExecutionsOutcome::Ran(ordered)
}

enum ApprovalResolution {
    Granted(Option<String>),
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
            Some(Command::Approve { id: got, prefix_rule }) if got == id => {
                return ApprovalResolution::Granted(prefix_rule);
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
async fn run_one(call: ToolCall, ctx: &ToolCtx, registry: &ToolRegistry, ev_tx: &UnboundedSender<Event>) -> String {
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

    let result: Result<ToolOutput, ToolError> = match registry.get(&name) {
        Some(tool) => tool.run(input, ctx).await,
        None => Err(ToolError::Failed(format!("unknown tool: {name}"))),
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(out) => {
            let _ = ev_tx.send(Event::ToolCallFinished {
                name,
                ok: out.ok,
                duration_ms,
                summary: out.summary,
            });
            out.result
        }
        Err(e) => {
            let _ = ev_tx.send(Event::ToolCallFinished {
                name,
                ok: false,
                duration_ms,
                summary: e.to_string(),
            });
            format!("ERROR: {e}")
        }
    }
}

fn input_preview(input: &serde_json::Value) -> String {
    let s = serde_json::to_string(input).unwrap_or_else(|_| "<unserializable>".into());
    let mut s: String = s.chars().take(INPUT_PREVIEW_CHARS).collect();
    if s.chars().count() == INPUT_PREVIEW_CHARS {
        s.push('\u{2026}');
    }
    s
}
