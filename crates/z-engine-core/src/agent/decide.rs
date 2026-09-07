//! Phase one of a tool round: decide every call the model made before
//! any of them runs — mode enforcement, the policy engine, and the
//! approval prompts, which surface one at a time.
//!
//! A refusal is not an absence. Plan mode blocking an edit, the user
//! declining a command, a turn aborted mid-prompt: each one puts text in
//! front of the model that shapes what it asks for next, exactly as a
//! tool result does. So each one claims its position on the tape here,
//! in the order the calls were decided, and is recorded as what it was.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use z_engine_provider::ToolCall;

use crate::perms::{Decision, PolicyEngine};
use crate::replay::ToolDisposition;
use crate::tools::{ToolCtx, ToolRegistry};

use super::events::{Command, Event, PermissionMode};
use super::execute::{input_preview, parse_input};
use super::state::LoopState;

/// What the model is told when a call was refused before it ran. Phase
/// three puts this in the transcript; the recorder tapes the same text,
/// because that is what the next request is built from.
pub(super) const REFUSAL: &str = "The user declined permission for this action. Do not retry it \
                                  unchanged; adjust your approach or explain what you need.";

/// What became of one call before execution.
pub(super) enum Verdict {
    Run,
    Refused,
}

/// One decided call: the ruling, and the tape position claimed for it.
pub(super) struct Decided {
    pub(super) verdict: Verdict,
    pub(super) sequence: Option<u64>,
}

pub(super) enum Decisions {
    Made(Vec<Decided>),
    /// The user ended the turn at an approval prompt; nothing runs.
    Aborted,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn decide_calls(
    calls: &[ToolCall],
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    state: &mut LoopState,
    abort_flag: &Arc<AtomicBool>,
    mode: &PermissionMode,
) -> Decisions {
    let mut decided: Vec<Decided> = Vec::with_capacity(calls.len());
    for (index, call) in calls.iter().enumerate() {
        // Claimed before the ruling, so the tape follows the order the
        // model asked in rather than the order tools happened to finish.
        let sequence = ctx.claim_tool_sequence();
        let name = &call.function.name;
        let input = parse_input(&call.function.arguments);

        // Mode enforcement precedes everything else.
        if *mode == PermissionMode::Plan && is_mutating(name) {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "plan mode blocked {name} — switch modes to apply changes"
            )));
            ctx.record_tool_outcome(sequence, name, ToolDisposition::PlanRefused, false, REFUSAL);
            decided.push(Decided {
                verdict: Verdict::Refused,
                sequence,
            });
            continue;
        }

        let decision = ctx
            .perms
            .lock()
            .map(|p| p.decide(name, &input))
            .unwrap_or(Decision::Gate);
        let verdict = match decision {
            Decision::Allow => Verdict::Run,
            Decision::Gate => {
                match gate(
                    call, &input, registry, ctx, cmd_rx, ev_tx, state, abort_flag, mode,
                )
                .await
                {
                    GateOutcome::Run => Verdict::Run,
                    GateOutcome::Denied => {
                        ctx.record_tool_outcome(
                            sequence,
                            name,
                            ToolDisposition::UserDenied,
                            false,
                            REFUSAL,
                        );
                        Verdict::Refused
                    }
                    GateOutcome::AbortTurn => {
                        // This call and every one after it is dropped;
                        // a round that vanishes from the tape would look
                        // like a round the model never asked for.
                        for (offset, abandoned) in calls[index..].iter().enumerate() {
                            let seq = if offset == 0 {
                                sequence
                            } else {
                                ctx.claim_tool_sequence()
                            };
                            ctx.record_tool_outcome(
                                seq,
                                &abandoned.function.name,
                                ToolDisposition::Abandoned,
                                false,
                                "[aborted]",
                            );
                        }
                        return Decisions::Aborted;
                    }
                }
            }
        };
        decided.push(Decided { verdict, sequence });
    }
    Decisions::Made(decided)
}

fn is_mutating(name: &str) -> bool {
    matches!(name, "bash" | "write_file" | "edit_file")
}

enum GateOutcome {
    Run,
    Denied,
    AbortTurn,
}

/// Ask — unless this mode already answers for the user.
#[allow(clippy::too_many_arguments)]
async fn gate(
    call: &ToolCall,
    input: &serde_json::Value,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    state: &mut LoopState,
    abort_flag: &Arc<AtomicBool>,
    mode: &PermissionMode,
) -> GateOutcome {
    let name = &call.function.name;
    // Auto-accept edits mode: file edits — and the common filesystem
    // bash set (mkdir/touch/mv/cp/rm/sed, Claude Code acceptEdits
    // parity) — skip the prompt.
    if *mode == PermissionMode::AutoAcceptEdits {
        if matches!(name.as_str(), "write_file" | "edit_file") {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "auto-accepted edit to {}",
                input.get("path").and_then(|v| v.as_str()).unwrap_or("?")
            )));
            return GateOutcome::Run;
        }
        if name == "bash"
            && input
                .get("command")
                .and_then(|v| v.as_str())
                .is_some_and(PolicyEngine::is_common_fs_command)
        {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "auto-accepted fs command: {}",
                input.get("command").and_then(|v| v.as_str()).unwrap_or("?")
            )));
            return GateOutcome::Run;
        }
    }

    let suggested_rule = (name == "bash").then(|| {
        PolicyEngine::suggested_rule(input.get("command").and_then(|v| v.as_str()).unwrap_or(""))
    });
    // Outside the project root? Then "persist" is disabled (spec section
    // 5) and the call always gates on future runs.
    let target_outside = input
        .get("path")
        .and_then(|v| v.as_str())
        .map(|p| ctx.is_outside_root(Path::new(p)))
        .unwrap_or(false);
    state.approval_counter += 1;
    let id = state.approval_counter;
    let detail = registry
        .get(name)
        .and_then(|t| t.approval_preview(input, ctx));
    let _ = ev_tx.send(Event::ApprovalRequired {
        id,
        tool: name.clone(),
        input_preview: input_preview(input),
        suggested_rule,
        detail_preview: detail,
        can_persist: !target_outside && name == "bash",
        bash_command: (name == "bash")
            .then(|| {
                input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .flatten(),
    });

    match wait_for_approval(id, cmd_rx, abort_flag).await {
        ApprovalResolution::Granted(decision) => {
            use crate::agent::events::ApprovalDecision;
            match decision {
                ApprovalDecision::Once => {}
                ApprovalDecision::AlwaysSession { rule } => {
                    if let Ok(mut p) = ctx.perms.lock() {
                        p.add_session_rule(rule);
                    }
                }
                ApprovalDecision::AlwaysPersist { rule } => {
                    match crate::config::persist_bash_rule(&ctx.project_root, &rule) {
                        Err(e) => {
                            tracing::warn!(error = %e, "failed persisting rule");
                            let _ = ev_tx
                                .send(Event::StatusNote(format!("could not persist rule: {e}")));
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
                }
            }
            GateOutcome::Run
        }
        ApprovalResolution::Denied => GateOutcome::Denied,
        ApprovalResolution::AbortTurn => GateOutcome::AbortTurn,
    }
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
