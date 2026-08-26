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

use std::collections::{BTreeMap, HashMap};
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
    AccumulatedToolCall, ChatMessage, ChatRequest, Client, ContentPart, ProviderError, StreamEvent,
    ToolCall, ToolCallAccumulator, Usage,
};
use crate::session::{SessionEvent, SessionWriter};
use crate::tools::{ToolCtx, ToolError, ToolOutput, ToolRegistry};

pub use events::{ApprovalDecision, Command, Event, PermissionMode};

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
    /// Explicit per-request output ceiling (max_tokens).
    pub max_output_tokens: u32,
    /// Lifecycle shell hooks (`session_start`, `turn_completed`).
    pub hooks: BTreeMap<String, String>,
    /// Auto-compaction trigger point as a percent of the budget.
    pub compact_at_percent: u8,
    /// Verbatim L2 tail size for compaction.
    pub keep_recent_messages: usize,
    /// Run the post-edit reviewer pass (spec section 9 v0.9).
    pub review_enabled: bool,
    /// External MCP stdio servers to register at startup (v0.9).
    pub mcp_servers: Vec<crate::mcp::McpServerConfig>,
    /// Tools auto-allowed without gating (e.g. trusted MCP externals).
    pub auto_allow_tools: Vec<String>,
    /// Starting permission mode (spec section 9 v1.1 parity).
    pub initial_mode: crate::agent::events::PermissionMode,
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
            max_output_tokens: 16_384,
            hooks: BTreeMap::new(),
            compact_at_percent: 92,
            keep_recent_messages: compact::DEFAULT_KEEP_RECENT,
            review_enabled: true,
            mcp_servers: Vec::new(),
            auto_allow_tools: Vec::new(),
            initial_mode: crate::agent::events::PermissionMode::Normal,
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
        self.submit_with_images(text, Vec::new());
    }

    /// Submit a task with attached images (data URLs) for vision models.
    pub fn submit_with_images(&self, text: impl Into<String>, images: Vec<String>) {
        let _ = self.cmd_tx.send(Command::SubmitMessage {
            text: text.into(),
            images,
        });
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

    /// Set the permission mode (Shift+Tab).
    pub fn set_mode(&self, mode: crate::agent::events::PermissionMode) {
        let _ = self.cmd_tx.send(Command::SetMode(mode));
    }

    /// Hot-switch the provider model (`/model <id>`).
    pub fn set_model(&self, model: impl Into<String>) {
        let _ = self.cmd_tx.send(Command::SetModel(model.into()));
    }

    /// Pick the reasoning effort for reasoning-capable models; `None`
    /// stops sending the parameter entirely.
    pub fn set_reasoning_effort(&self, effort: Option<String>) {
        let _ = self.cmd_tx.send(Command::SetReasoningEffort(effort));
    }

    /// Force context compaction now (`/compact`).
    pub fn compact(&self) {
        let _ = self.cmd_tx.send(Command::Compact);
    }

    /// Dump the current L1 notes (`/notes`).
    pub fn request_notes(&self) {
        let _ = self.cmd_tx.send(Command::RequestNotes);
    }

    /// Rewind: restore files touched by the last checkpointed turn.
    pub fn revert_last_turn(&self) {
        let _ = self.cmd_tx.send(Command::RevertLastTurn);
    }

    /// Per-message revert: restore all file changes from run-turn `keep`
    /// (the user message being reverted) and every later turn.
    pub fn revert_to_turn(&self, keep: u64) {
        let _ = self.cmd_tx.send(Command::RevertToTurn(keep));
    }

    /// `!<cmd>` local shell passthrough (never reaches the model).
    pub fn shell(&self, cmd: impl Into<String>) {
        let _ = self.cmd_tx.send(Command::Shell(cmd.into()));
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
    let max_output = cfg.max_output_tokens;
    let sub_abort = Arc::clone(&abort_flag);
    let runner: crate::tools::SubAgentRunner = Arc::new(move |prompt: String, max_rounds: u32| {
        let client = sub_client.clone();
        let model = model.clone();
        let root = project_root.clone();
        let tmp = tmp_dir.clone();
        let abort = Arc::clone(&sub_abort);
        Box::pin(async move {
            run_isolated(
                client, model, root, tmp, abort, &prompt, max_rounds, max_output,
            )
            .await
        })
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
    /// The active task text (reviewer prompt context).
    current_task: String,
    /// Reasoning effort for reasoning-capable models; `None` = omit param.
    reasoning_effort: Option<String>,
}

impl LoopState {
    fn estimate_working(&self) -> u64 {
        let mut bytes = 0usize;
        for m in &self.working {
            let text = match m {
                ChatMessage::System { content }
                | ChatMessage::User { content }
                | ChatMessage::Tool { content, .. } => content.as_str(),
                ChatMessage::UserMulti { content } => {
                    let mut n = 0usize;
                    for part in content {
                        if let ContentPart::Text { text } = part {
                            n += text.len();
                        }
                        if let ContentPart::ImageUrl { image_url } = part {
                            // Rough vision-token proxy: data URLs are big.
                            n += image_url.url.len() / 4;
                        }
                    }
                    // handled below via push
                    bytes += n;
                    continue;
                }
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
    mut cfg: LoopConfig,
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
    // Register external MCP tools (spec section 9 v0.9). Failures are
    // logged and skipped: a broken server must not kill the session.
    let mut registry = registry;
    for srv_cfg in &cfg.mcp_servers {
        let conn = crate::mcp::McpConnection::new(
            &srv_cfg.name,
            &srv_cfg.command,
            &srv_cfg.args,
            &cfg.project_root,
        );
        match conn.ensure().await {
            Err(e) => {
                tracing::warn!(server = %srv_cfg.name, error = %e, "mcp server failed to start")
            }
            Ok(()) => {
                for info in conn.list_tools().await {
                    let tool = crate::mcp::tool_adapter::McpTool {
                        conn: Arc::new(conn.clone()),
                        info,
                    };
                    registry.register(Arc::new(tool));
                }
                let _ = ev_tx.send(Event::StatusNote(format!(
                    "registered mcp server '{}'",
                    srv_cfg.name
                )));
            }
        }
    }
    let notes = Arc::new(Mutex::new(NotesStore::default()));
    let (output_tx, mut output_rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::tools::ToolOutputChunk>();
    let mut ctx = ToolCtx::new(
        cfg.project_root.clone(),
        Arc::clone(&perms),
        cfg.tmp_dir.clone(),
    )
    .with_task_runner(runner);
    ctx.output_tx = Arc::new(output_tx);
    ctx.notes = Arc::clone(&notes);

    // Forward live tool output to the UI.
    {
        let ev_tx2 = ev_tx.clone();
        tokio::spawn(async move {
            while let Some(chunk) = output_rx.recv().await {
                let _ = ev_tx2.send(Event::ToolOutputDelta {
                    tool_name: chunk.tool_name,
                    text: chunk.text,
                });
            }
        });
    }
    if let Ok(mut p) = perms.lock() {
        for t in &cfg.auto_allow_tools {
            p.allow_tool(t);
        }
    }
    run_hook(&cfg.hooks, "session_start", &cfg.project_root, &ev_tx).await;
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
    let meter =
        BudgetMeter::new(cfg.max_context_tokens).with_compact_percent(cfg.compact_at_percent);

    let mut state = LoopState {
        working: Vec::new(),
        approval_counter: 0,
        last_usage: Usage::default(),
        force_compact: false,
        repo_map_text: None,
        current_task: String::new(),
        reasoning_effort: None,
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
            Command::SubmitMessage {
                text: user_text,
                images,
            } => {
                state.current_task = user_text.clone();
                ctx.begin_checkpoint_turn();
                if let Some(w) = recorder.as_mut() {
                    let _ = w.record(&SessionEvent::UserMsg {
                        text: user_text.clone(),
                        images: images.clone(),
                    });
                }
                state
                    .working
                    .push(ChatMessage::user_with_images(&user_text, &images));
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
                        run_hook(&cfg.hooks, "turn_completed", &cfg.project_root, &ev_tx).await;
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
            Command::SetMode(m) => {
                cfg.initial_mode = m;
                let _ = ev_tx.send(Event::StatusNote(format!("mode: {}", m.label())));
            }
            Command::SetModel(id) => {
                cfg.model = id.clone();
                let _ = ev_tx.send(Event::StatusNote(format!("model set to {id}")));
            }
            Command::SetReasoningEffort(effort) => {
                state.reasoning_effort = effort.clone();
                let note = match effort {
                    Some(e) => format!("reasoning effort: {e}"),
                    None => "reasoning effort: default (param omitted)".to_string(),
                };
                let _ = ev_tx.send(Event::StatusNote(note));
            }
            Command::Shell(cmd) => match registry.get("bash") {
                Some(bash) => run_shell_passthrough(&cmd, bash, &ctx, &ev_tx).await,
                None => {
                    let _ = ev_tx.send(Event::StatusNote("shell unavailable".into()));
                }
            },
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
            Command::RevertLastTurn => {
                let out = ctx.checkpoints.revert_last_turn();
                use std::sync::atomic::Ordering;
                ctx.repo_map_dirty.store(true, Ordering::Relaxed);
                let root = &cfg.project_root;
                let names: Vec<String> = out
                    .restored
                    .iter()
                    .map(|p| {
                        p.strip_prefix(root)
                            .unwrap_or(p)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect();
                let note = if out.restored.is_empty() && out.errors.is_empty() {
                    "rewind: nothing to revert".to_string()
                } else {
                    let mut s = format!("rewound {} file(s)", out.restored.len());
                    if !names.is_empty() {
                        let shown: Vec<String> = names.iter().take(3).cloned().collect();
                        s.push_str(": ");
                        s.push_str(&shown.join(", "));
                        if names.len() > 3 {
                            s.push_str(&format!(" +{}", names.len() - 3));
                        }
                    }
                    if !out.errors.is_empty() {
                        s.push_str(&format!(" ({} failed)", out.errors.len()));
                    }
                    s
                };
                for e in &out.errors {
                    tracing::warn!(error = %e, "revert restore failed");
                }
                let _ = ev_tx.send(Event::StatusNote(note));
            }
            Command::RevertToTurn(keep) => {
                let out = ctx.checkpoints.revert_to_turn(keep);
                use std::sync::atomic::Ordering;
                ctx.repo_map_dirty.store(true, Ordering::Relaxed);
                let root = &cfg.project_root;
                let names: Vec<String> = out
                    .restored
                    .iter()
                    .map(|p| {
                        p.strip_prefix(root)
                            .unwrap_or(p)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect();
                let note = if out.restored.is_empty() && out.errors.is_empty() {
                    format!(
                        "rewind: no file changes recorded at or after turn {keep} \
                         (checkpoints do not survive an app restart)"
                    )
                } else {
                    let mut s = format!(
                        "rewound {} file(s) to before turn {keep}",
                        out.restored.len()
                    );
                    if out.evicted_gaps {
                        s.push_str(" (warning: some older checkpoints were evicted and cannot be restored)");
                    }
                    if !names.is_empty() {
                        let shown: Vec<String> = names.iter().take(3).cloned().collect();
                        s.push_str(": ");
                        s.push_str(&shown.join(", "));
                        if names.len() > 3 {
                            s.push_str(&format!(" +{}", names.len() - 3));
                        }
                    }
                    if !out.errors.is_empty() {
                        s.push_str(&format!(" ({} failed)", out.errors.len()));
                    }
                    s
                };
                for e in &out.errors {
                    tracing::warn!(error = %e, "revert-to-turn restore failed");
                }
                let _ = ev_tx.send(Event::StatusNote(note));
            }
            _ => { /* stale Approve/Deny/Abort while idle are ignored */ }
        }
    }
    tracing::debug!("agent task exiting");
}

/// Run a lifecycle hook (`[hooks]` in config.toml) with a hard timeout.
/// stdout becomes a status note; failures are reported but never fatal.
async fn run_hook(
    hooks: &BTreeMap<String, String>,
    event: &str,
    root: &Path,
    ev_tx: &UnboundedSender<Event>,
) {
    let Some(cmd) = hooks.get(event) else {
        return;
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(root)
            .env("HARNESS_EVENT", event)
            .env("HARNESS_PROJECT_ROOT", root)
            .output(),
    )
    .await;
    match output {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !text.is_empty() {
                let _ = ev_tx.send(Event::StatusNote(format!("[hook:{event}] {text}")));
            }
        }
        Ok(Ok(out)) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let _ = ev_tx.send(Event::StatusNote(format!(
                "[hook:{event}] failed (exit {}): {}",
                out.status,
                err.chars().take(160).collect::<String>()
            )));
        }
        Ok(Err(e)) => {
            tracing::warn!(event, error = %e, "hook spawn failed");
        }
        Err(_) => {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "[hook:{event}] timed out after 15s"
            )));
        }
    }
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

/// Post-edit reviewer (spec section 9 v0.9): a side-request that audits
/// this round's diffs against the original task. Returns findings text, or
/// None for "no findings" / transport failure (never blocks the turn).
async fn run_review(
    client: &Client,
    model: &str,
    task: &str,
    edit_results: &[String],
) -> Option<String> {
    const REVIEWER_SYSTEM: &str = "You are the code reviewer inside the harness coding agent.\nGiven the user's task and the diffs just applied, list CONCRETE problems: bugs, missed requirements, broken invariants, dangerous side effects.\nIgnore style. Reference files/lines when possible.\nIf everything is fine, reply with exactly: NO_FINDINGS";

    let mut body = String::from("# Original task\n");
    body.push_str(task.trim());
    body.push_str("\n\n# Edits applied this round\n");
    for (i, entry) in edit_results.iter().enumerate() {
        let clipped: String = entry.chars().take(3_000).collect();
        let _ =
            std::fmt::Write::write_fmt(&mut body, format_args!("\n## Edit {}\n{clipped}\n", i + 1));
    }

    let req = ChatRequest::new(
        model.to_string(),
        vec![
            ChatMessage::system(REVIEWER_SYSTEM),
            ChatMessage::user(body),
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
                tracing::warn!(error = %e, "reviewer stream failed");
                return None;
            }
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() || out.contains("NO_FINDINGS") {
        None
    } else {
        Some(out)
    }
}

/// `!<cmd>` passthrough: run locally through the bash tool so output is
/// truncated/spilled consistently; never touches the model.
async fn run_shell_passthrough(
    cmd: &str,
    bash: Arc<dyn crate::tools::Tool>,
    ctx: &ToolCtx,
    ev_tx: &UnboundedSender<Event>,
) {
    use serde_json::json;
    let input = json!({"command": cmd.to_string()});
    let out = bash.run(input, ctx).await;
    let text = match out {
        Ok(o) => o.result,
        Err(e) => format!("ERROR: {e}"),
    };
    for line in text.lines().take(40) {
        let _ = ev_tx.send(Event::StatusNote(format!("$ {line}")));
    }
}

/// Isolated sub-agent loop (spec section 9 v0.7): read-only toolset, own
/// transcript, bounded rounds; returns the final assistant text only.
#[allow(clippy::too_many_arguments)]
async fn run_isolated(
    client: Client,
    model: String,
    project_root: PathBuf,
    tmp_dir: PathBuf,
    abort: Arc<AtomicBool>,
    prompt: &str,
    max_rounds: u32,
    max_output_tokens: u32,
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
        let request = ChatRequest::new(model.clone(), messages.clone())
            .with_tools(registry.defs())
            .with_max_tokens(max_output_tokens);
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
                    Some(Ok(StreamEvent::ReasoningDelta(_))) => {}
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
                    let raw_short: String = raw_arguments.chars().take(160).collect();
                    synthetic_errors.push((
                        id.clone(),
                        format!(
                            "ERROR: arguments not valid JSON ({reason}). You sent: {raw_short}"
                        ),
                    ));
                    // Keep the call on the wire so the error pairs up.
                    complete_calls.push(ToolCall {
                        id,
                        function: crate::provider::FunctionCall {
                            name: name.unwrap_or_default(),
                            arguments: raw_arguments,
                        },
                    });
                }
                AccumulatedToolCall::MissingId { index } => {
                    messages.push(ChatMessage::user(format!(
                        "[harness] tool call index {index} had no id; skipped."
                    )));
                }
            }
        }

        // Assistant message must precede its tool results on the wire.
        messages.push(ChatMessage::Assistant {
            content: (!text.is_empty()).then_some(text),
            tool_calls: complete_calls.clone(),
        });
        for (id, err) in synthetic_errors {
            messages.push(ChatMessage::tool_result(id, err));
        }

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

        let mut request =
            ChatRequest::new(cfg.model.clone(), request_messages).with_tools(registry.defs());
        // Explicit output ceiling: without it gateways assume the model
        // maximum and pre-charge credits against that worst case.
        request = request.with_max_tokens(cfg.max_output_tokens);
        if let Some(effort) = state.reasoning_effort.clone() {
            request = request.with_reasoning_effort(effort);
        }
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
        // Calls whose arguments never parsed: not executed, but kept on
        // the wire so the synthetic error tool-result has a matching
        // assistant `tool_calls` entry (strict OpenAI-compatible APIs
        // reject unpaired tool results with 400 — poisoning the session).
        let mut wire_only_calls: Vec<ToolCall> = Vec::new();
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
                        id.clone(),
                        format!(
                            "ERROR: arguments were not valid JSON ({reason}). You sent: {raw_short}"
                        ),
                    ));
                    wire_only_calls.push(ToolCall {
                        id,
                        function: crate::provider::FunctionCall {
                            name: name.unwrap_or_default(),
                            arguments: raw_arguments,
                        },
                    });
                }
                AccumulatedToolCall::MissingId { index } => {
                    tracing::warn!(index, "tool-call delta without id; skipped");
                    state.working.push(ChatMessage::user(format!(
                        "[harness] a tool call (index {index}) arrived without an id and was skipped."
                    )));
                }
            }
        }

        let mut all_wire_calls = complete_calls.clone();
        all_wire_calls.extend(wire_only_calls);
        if let Some(w) = recorder.as_mut() {
            let _ = w.record(&SessionEvent::AssistantMsg {
                content: (!text.is_empty()).then(|| text.clone()),
                tool_calls: all_wire_calls
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
            tool_calls: all_wire_calls,
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
            &cfg.initial_mode,
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

                // Reviewer pass (spec section 9 v0.9): after a batch that
                // edited files, ask a side-model to audit the diffs.
                let journal = ctx.take_edit_journal();
                if cfg.review_enabled && !journal.is_empty() {
                    match run_review(client, &cfg.model, &state.current_task, &journal).await {
                        Some(findings) => {
                            let _ =
                                ev_tx.send(Event::StatusNote("reviewer posted findings".into()));
                            state
                                .working
                                .push(ChatMessage::user(format!("[harness reviewer]\n{findings}")));
                        }
                        None => {
                            let _ = ev_tx.send(Event::StatusNote("reviewer: no findings".into()));
                        }
                    }
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
                        StreamEvent::ReasoningDelta(r) => {
                            let _ = ev_tx.send(Event::ReasoningDelta(r));
                        }
                        StreamEvent::ToolCallDelta { index, id, name, args_delta } => {
                            acc.absorb(index, id.as_deref(), name.as_deref(), &args_delta);
                        }
                        StreamEvent::Usage(u) => {
                            // Latest prompt size + running completion total —
                            // the budget-pressure signal for v0.3's compactor.
                            // Replace (not max): after compaction the true
                            // prompt shrinks, and keeping the stale larger
                            // value would spuriously re-trigger compaction.
                            usage_out.prompt_tokens = u.prompt_tokens;
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

#[allow(clippy::too_many_arguments)]
async fn execute_calls(
    calls: Vec<ToolCall>,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    state: &mut LoopState,
    abort_flag: &Arc<AtomicBool>,
    mode: &crate::agent::events::PermissionMode,
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

        // Mode enforcement precedes everything else.
        let mutating = matches!(
            call.function.name.as_str(),
            "bash" | "write_file" | "edit_file"
        );
        if *mode == crate::agent::events::PermissionMode::Plan && mutating {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "plan mode blocked {} — switch modes to apply changes",
                call.function.name
            )));
            verdicts.push(Verdict::Denied);
            continue;
        }

        verdicts.push(match decision {
            Decision::Allow => Verdict::Run,
            Decision::Gate => {
                // Auto-accept edits mode: file edits — and the common
                // filesystem bash set (mkdir/touch/mv/cp/rm/sed, Claude
                // Code acceptEdits parity) — skip the prompt.
                if *mode == crate::agent::events::PermissionMode::AutoAcceptEdits
                    && matches!(call.function.name.as_str(), "write_file" | "edit_file")
                {
                    let _ = ev_tx.send(Event::StatusNote(format!(
                        "auto-accepted edit to {}",
                        input.get("path").and_then(|v| v.as_str()).unwrap_or("?")
                    )));
                    verdicts.push(Verdict::Run);
                    continue;
                }
                if *mode == crate::agent::events::PermissionMode::AutoAcceptEdits
                    && call.function.name == "bash"
                    && input
                        .get("command")
                        .and_then(|v| v.as_str())
                        .is_some_and(PolicyEngine::is_common_fs_command)
                {
                    let _ = ev_tx.send(Event::StatusNote(format!(
                        "auto-accepted fs command: {}",
                        input.get("command").and_then(|v| v.as_str()).unwrap_or("?")
                    )));
                    verdicts.push(Verdict::Run);
                    continue;
                }
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
                    bash_command: (call.function.name == "bash")
                        .then(|| {
                            input
                                .get("command")
                                .and_then(|v| v.as_str())
                                .map(str::to_string)
                        })
                        .flatten(),
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
