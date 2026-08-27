//! The background agent task: startup wiring (MCP, LSP, notes, hooks) and
//! the idle command loop that dispatches single turns.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use z_engine_provider::{ChatMessage, ChatRequest, Client, Usage};

use crate::context::{
    budget::BudgetMeter,
    notes::{NotesInput, NotesStore},
};
use crate::perms::PolicyEngine;
use crate::session::{SessionEvent, SessionWriter};
use crate::tools::{ToolCtx, ToolRegistry};

use super::LoopConfig;
use super::events::{Command, Event};
use super::handle::ResumeState;
use super::prompt_inspect::PromptInspect;
use super::revert::{revert_last_turn, revert_to_turn, trim_working_before_user_turn};
use super::side_requests::generate_session_title;
use super::state::LoopState;
use super::system_prompt::l0_message;
use super::turn::{TurnOutcome, run_turn};

#[allow(clippy::too_many_arguments)]
pub(super) async fn agent_task(
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
    last_prompt: Arc<Mutex<Option<PromptInspect>>>,
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
                state.current_task = user_text.clone();
                ctx.begin_checkpoint_turn();
                if let Some(w) = recorder.as_mut() {
                    let _ = w.record(&SessionEvent::UserMsg {
                        text: user_text.clone(),
                        images: images.clone(),
                    });
                }
                if !titled {
                    titled = true;
                    let client = client.clone();
                    let model = cfg.model.clone();
                    let prompt = user_text.clone();
                    let ev_tx2 = ev_tx.clone();
                    let path = recorder.as_ref().map(|w| w.path.clone());
                    tokio::spawn(async move {
                        let title = generate_session_title(&client, &model, &prompt)
                            .await
                            .unwrap_or_else(|| crate::session::fallback_title(&prompt));
                        if let Some(path) = path {
                            if let Ok(mut w) = SessionWriter::append_to(&path) {
                                let _ = w.record(&SessionEvent::Title {
                                    text: title.clone(),
                                });
                            }
                        }
                        let _ = ev_tx2.send(Event::SessionTitle { text: title });
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
                        if let Some(w) = recorder.as_mut() {
                            let _ = w.record(&SessionEvent::TurnEnd {
                                outcome: "completed".into(),
                            });
                        }
                        let _ = ev_tx.send(Event::TurnCompleted {
                            prompt_tokens: state.last_usage.prompt_tokens,
                            completion_tokens: state.last_usage.completion_tokens,
                        });
                        run_hook(&cfg.hooks, "turn_completed", &cfg.project_root, &ev_tx).await;
                    }
                    TurnOutcome::Aborted => {
                        abort_flag.store(false, Ordering::Relaxed);
                        if let Some(w) = recorder.as_mut() {
                            let _ = w.record(&SessionEvent::TurnEnd {
                                outcome: "aborted".into(),
                            });
                        }
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
                revert_last_turn(&ctx, &cfg.project_root, &ev_tx);
            }
            Command::RevertToTurn(keep) => {
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
            .env("ZENGINE_EVENT", event)
            .env("HARNESS_EVENT", event)
            .env("ZENGINE_PROJECT_ROOT", root)
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

/// Wait for a meaningful action; channel close / Shutdown ends the task.
async fn next_action(cmd_rx: &mut UnboundedReceiver<Command>) -> Option<Command> {
    match cmd_rx.recv().await {
        None | Some(Command::Shutdown) => None,
        Some(c) => Some(c),
    }
}
