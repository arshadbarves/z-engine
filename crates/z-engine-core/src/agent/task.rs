//! The background agent task: startup wiring (MCP, LSP, notes, hooks) and
//! the idle command loop that dispatches single turns.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use z_engine_provider::{ChatMessage, ChatRequest, Client, Usage};

use crate::context::{
    budget::BudgetMeter,
    notes::{NotesInput, NotesStore},
};
use crate::perms::PolicyEngine;
use crate::session::SessionWriter;
use crate::tools::{CheckpointStore, ToolCtx, ToolRegistry};

use super::LoopConfig;
use super::events::{Command, Event};
use super::handle::ResumeState;
use super::hooks::{run_hook, run_shell_passthrough};
use super::prompt_inspect::PromptInspect;
use super::revert::{revert_last_turn, revert_to_turn, trim_working_before_user_turn};
use super::state::LoopState;
use super::system_prompt::l0_message;
use super::{submission, task_completion};

#[allow(clippy::too_many_arguments)]
pub(super) async fn agent_task(
    mut cfg: LoopConfig,
    mut client: Client,
    perms: Arc<Mutex<PolicyEngine>>,
    registry: ToolRegistry,
    mut cmd_rx: UnboundedReceiver<Command>,
    ev_tx: UnboundedSender<Event>,
    resume: Option<ResumeState>,
    mut recorder: Option<SessionWriter>,
    runner: crate::tools::SubAgentRunner,
    abort_flag: Arc<AtomicBool>,
    last_prompt: Arc<Mutex<Option<PromptInspect>>>,
    checkpoints: Arc<CheckpointStore>,
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
                    if registry.get(&info.name).is_some()
                        || matches!(
                            info.name.as_str(),
                            "go_to_definition" | "find_references" | "lsp_diagnostics"
                        )
                    {
                        let _ = ev_tx.send(Event::StatusNote(format!(
                            "MCP tool '{}' was not registered because its name is already reserved",
                            info.name
                        )));
                        continue;
                    }
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
    if let Ok(mut slot) = last_prompt.lock() {
        *slot = Some(PromptInspect::preview(&cfg, registry.defs()));
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
    ctx.checkpoints = checkpoints;
    ctx.abort = Arc::clone(&abort_flag);
    ctx.evidence_dir = recorder
        .as_ref()
        .map(|writer| writer.path.with_extension("artifacts"));
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
        last_prompt,
    };
    // Seed from a previous session's transcript (resume).
    let mut titled = resume.is_some();
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
        if !state.working.is_empty() {
            let mut msgs = vec![l0_message(&cfg)];
            if let Some(block) = notes.lock().ok().and_then(|n| n.render_block()) {
                msgs.push(ChatMessage::system(block));
            }
            msgs.extend(state.working.iter().cloned());
            if let Ok(mut slot) = state.last_prompt.lock() {
                *slot = Some(PromptInspect::from_request(
                    &ChatRequest::new(cfg.model.clone(), msgs).with_tools(registry.defs()),
                    true,
                ));
            }
        }
    }

    while let Some(command) = next_action(&mut cmd_rx).await {
        match command {
            Command::SubmitMessage {
                text: user_text,
                images,
            } => {
                submission::submit(
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
                    &mut titled,
                    user_text,
                    images,
                )
                .await;
            }
            Command::SetMode(m) => {
                cfg.initial_mode = m;
                let _ = ev_tx.send(Event::StatusNote(format!("mode: {}", m.label())));
            }
            Command::SetModel(id) => {
                cfg.model = id.clone();
                let _ = ev_tx.send(Event::StatusNote(format!("model set to {id}")));
            }
            Command::SetApiKey(key) => {
                client.set_api_key(key.clone());
                cfg.api_key = key;
                let _ = ev_tx.send(Event::StatusNote("api key updated".into()));
            }
            Command::SetProvider { base_url, api_key } => {
                match Client::new(&base_url, api_key.clone()) {
                    Ok(next) => {
                        client = next;
                        cfg.base_url = base_url;
                        cfg.api_key = api_key;
                        let _ = ev_tx.send(Event::StatusNote("provider updated".into()));
                    }
                    Err(error) => {
                        let _ = ev_tx.send(Event::Error(error.to_string()));
                    }
                }
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
                Some(bash) => {
                    if let Err(error) =
                        task_completion::invalidate_and_publish(&ctx, &mut recorder, &ev_tx)
                    {
                        let _ = ev_tx.send(Event::Error(error.to_string()));
                        continue;
                    }
                    run_shell_passthrough(&cmd, bash, &ctx, &ev_tx).await;
                }
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
                if let Err(error) =
                    task_completion::invalidate_and_publish(&ctx, &mut recorder, &ev_tx)
                {
                    let _ = ev_tx.send(Event::Error(error.to_string()));
                    continue;
                }
                revert_last_turn(&ctx, &cfg.project_root, &ev_tx);
            }
            Command::RevertToTurn(keep) => {
                if keep == 0 {
                    if let Ok(mut task_guard) = ctx.task.lock() {
                        *task_guard = None;
                    }
                } else if let Err(error) =
                    task_completion::invalidate_and_publish(&ctx, &mut recorder, &ev_tx)
                {
                    let _ = ev_tx.send(Event::Error(error.to_string()));
                    continue;
                }
                revert_to_turn(&ctx, &cfg.project_root, keep, &ev_tx);
                trim_working_before_user_turn(&mut state.working, keep);
                if let Some(w) = recorder.as_mut() {
                    if let Err(e) = crate::session::trim_file_before_user_turn(&w.path, keep) {
                        tracing::warn!(error = %e, "session trim failed");
                    } else if let Err(e) = w.reopen() {
                        tracing::warn!(error = %e, "session writer reopen failed");
                    }
                }
                let _ = ev_tx.send(Event::TranscriptTrimmed { keep_turn: keep });
            }
            _ => { /* stale Approve/Deny/Abort while idle are ignored */ }
        }
    }
    tracing::debug!("agent task exiting");
}

/// Wait for a meaningful action; channel close / Shutdown ends the task.
async fn next_action(cmd_rx: &mut UnboundedReceiver<Command>) -> Option<Command> {
    match cmd_rx.recv().await {
        None | Some(Command::Shutdown) => None,
        Some(c) => Some(c),
    }
}
