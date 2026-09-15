//! Single-turn pipeline: pressure management, request assembly (L0 +
//! repo map + notes + working set), stream consumption, assistant-message
//! reconstruction, tool execution, and the post-edit reviewer pass.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use z_engine_provider::{AccumulatedToolCall, ChatMessage, Client, ToolCall, ToolCallAccumulator};

use crate::context::{
    budget::{BudgetMeter, Pressure},
    notes::NotesStore,
};
use crate::session::{SessionEvent, SessionWriter};
use crate::tools::{ToolCtx, ToolRegistry};
use crate::verification::WorkspaceSnapshot;

use super::LoopConfig;
use super::compaction::{compact_working_set, elide_marked_outputs};
use super::events::{Command, Event};
use super::execute::{ExecutionsOutcome, execute_calls};
use super::review::{ReviewOutcome, run_review};
use super::state::LoopState;
use super::stream::{StreamOutcome, consume_stream};

/// Safety valve against genuinely runaway loops. Spec says "no hard turn
/// cap"; 500 consecutive tool rounds is far beyond any real task and only
/// guards pathological provider behavior. Recorded in docs/deviations.md.
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 500;

pub(super) enum TurnOutcome {
    Completed,
    Aborted,
    Failed(String),
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_turn(
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
    baseline: Option<&WorkspaceSnapshot>,
) -> TurnOutcome {
    let mut rounds: u32 = 0;
    let mut malformed_rounds: u32 = 0;
    let mut supervisor = match z_engine_runtime::Supervisor::new(cfg.max_task_continuations) {
        Ok(supervisor) => supervisor,
        Err(error) => return TurnOutcome::Failed(error.to_string()),
    };

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
            if let Err(error) =
                compact_working_set(client, cfg, state, notes, ev_tx, recorder, abort_flag).await
            {
                return if abort_flag.load(Ordering::Relaxed) {
                    TurnOutcome::Aborted
                } else {
                    TurnOutcome::Failed(format!("Could not compact context: {error}"))
                };
            }
            state.force_compact = false;
        } else if meter.level(state.pressure_tokens()) == Pressure::Warn {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "context at {} tokens ({}% of budget)",
                state.pressure_tokens(),
                state.pressure_tokens() * 100 / u64::from(meter.max_tokens.max(1))
            )));
        }
        // Eager droppable elision — every round, pressure or not.
        match elide_marked_outputs(&mut state.working, notes, &cfg.tmp_dir) {
            Ok(0) => {}
            Ok(count) => {
                let _ = ev_tx.send(Event::StatusNote(format!(
                    "dropped {count} marked tool output(s) from context"
                )));
            }
            Err(error) => return TurnOutcome::Failed(error.to_string()),
        }
        let request = match super::request::assemble(cfg, registry, ctx, state, notes, ev_tx) {
            Ok(request) => request,
            Err(error) => return TurnOutcome::Failed(error.to_string()),
        };
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
        let mut protocol_error = false;

        for call in finalized {
            match call {
                AccumulatedToolCall::Complete(c) => complete_calls.push(c),
                AccumulatedToolCall::MalformedArguments {
                    id,
                    name,
                    raw_arguments,
                    reason,
                } => {
                    protocol_error = true;
                    if let Err(error) = super::task_completion::invalidate(ctx) {
                        return TurnOutcome::Failed(error.to_string());
                    }
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
                        function: z_engine_provider::FunctionCall {
                            name: name.unwrap_or_default(),
                            arguments: raw_arguments,
                        },
                    });
                }
                AccumulatedToolCall::MissingId { index } => {
                    protocol_error = true;
                    if let Err(error) = super::task_completion::invalidate(ctx) {
                        return TurnOutcome::Failed(error.to_string());
                    }
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
            if let Err(error) = w.record(&SessionEvent::AssistantMsg {
                content: (!text.is_empty()).then(|| text.clone()),
                tool_calls: all_wire_calls
                    .iter()
                    .map(|c| crate::session::PersistedToolCall {
                        id: c.id.clone(),
                        name: c.function.name.clone(),
                        arguments: c.function.arguments.clone(),
                    })
                    .collect(),
            }) {
                return TurnOutcome::Failed(format!(
                    "Could not record assistant response: {error}"
                ));
            }
        }
        state.working.push(ChatMessage::Assistant {
            content: (!text.is_empty()).then_some(text),
            tool_calls: all_wire_calls,
        });
        for (id, content) in synthetic_errors {
            if let Some(w) = recorder.as_mut() {
                if let Err(error) = w.record(&SessionEvent::ToolResult {
                    tool_call_id: id.clone(),
                    content: content.clone(),
                }) {
                    return TurnOutcome::Failed(format!("Could not record tool error: {error}"));
                }
            }
            state.working.push(ChatMessage::tool_result(id, content));
        }

        if complete_calls.is_empty() {
            if protocol_error {
                malformed_rounds += 1;
                if malformed_rounds <= 2 {
                    continue;
                }
                return TurnOutcome::Failed(
                    "Model emitted malformed tool calls after two correction attempts.".into(),
                );
            }
            if cfg.max_task_continuations > 0 {
                match super::supervision::at_boundary(
                    &mut supervisor,
                    ctx,
                    baseline,
                    recorder,
                    ev_tx,
                )
                .await
                {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(error) => return TurnOutcome::Failed(error.to_string()),
                }
            }
            if abort_flag.load(Ordering::Relaxed) {
                return TurnOutcome::Aborted;
            }
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
            recorder,
        )
        .await
        {
            ExecutionsOutcome::Ran(results) => {
                for (call_id, content) in results {
                    if let Some(w) = recorder.as_mut() {
                        if let Err(error) = w.record(&SessionEvent::ToolResult {
                            tool_call_id: call_id.clone(),
                            content: content.clone(),
                        }) {
                            return TurnOutcome::Failed(format!(
                                "Could not record tool result: {error}"
                            ));
                        }
                    }
                    if let Err(error) = super::task_completion::publish(ctx, recorder, ev_tx) {
                        return TurnOutcome::Failed(error.to_string());
                    }
                    state
                        .working
                        .push(ChatMessage::tool_result(call_id, content));
                }

                // Reviewer pass (spec section 9 v0.9): after a batch that
                // edited files, ask a side-model to audit the diffs.
                let journal = ctx.take_edit_journal();
                if cfg.review_enabled && !journal.is_empty() {
                    match run_review(
                        client,
                        &cfg.model,
                        &state.current_task,
                        &journal,
                        abort_flag,
                    )
                    .await
                    {
                        ReviewOutcome::Findings(findings) => {
                            let _ =
                                ev_tx.send(Event::StatusNote("reviewer posted findings".into()));
                            state
                                .working
                                .push(ChatMessage::user(
                                    serde_json::json!({"kind": "review_findings", "findings": findings}).to_string()
                                ));
                        }
                        ReviewOutcome::NoFindings => {
                            let _ = ev_tx.send(Event::StatusNote("reviewer: no findings".into()));
                        }
                        ReviewOutcome::Cancelled => return TurnOutcome::Aborted,
                        ReviewOutcome::Unavailable(reason) => {
                            let _ = ev_tx.send(Event::StatusNote(reason.clone()));
                            if let Err(error) = super::task_completion::block(ctx, reason.clone()) {
                                return TurnOutcome::Failed(error.to_string());
                            }
                            state.working.push(ChatMessage::user(
                                serde_json::json!({"kind": "review_unavailable", "reason": reason})
                                    .to_string(),
                            ));
                        }
                    }
                }
            }
            ExecutionsOutcome::Aborted => return TurnOutcome::Aborted,
            ExecutionsOutcome::Failed(error) => return TurnOutcome::Failed(error),
        }
    }
}
