//! Permission gating and execution: decide every call up front (mode
//! enforcement, policy engine, approvals), run safe tools concurrently,
//! and map results/errors into transcript entries.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use z_engine_provider::ToolCall;

use crate::perms::{Decision, PolicyEngine};
use crate::tools::{ToolCtx, ToolError, ToolOutput, ToolRegistry};

use super::events::{Command, Event};
use super::state::LoopState;

const INPUT_PREVIEW_CHARS: usize = 160;

pub(super) enum ExecutionsOutcome {
    /// `(tool_call_id, transcript content)` in original call order.
    Ran(Vec<(String, String)>),
    Aborted,
}

enum Verdict {
    Run,
    Denied,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_calls(
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
                                        "rule \"{rule}\" persisted to .z-engine/config.toml"
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

pub(super) fn parse_input(arguments: &str) -> serde_json::Value {
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
