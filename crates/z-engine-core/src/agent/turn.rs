//! Single-turn pipeline: pressure management, request assembly (L0 +
//! repo map + notes + working set), stream consumption, assistant-message
//! reconstruction, tool execution, and the post-edit reviewer pass.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use z_engine_provider::{
    AccumulatedToolCall, ChatMessage, ChatProvider, ChatRequest, ToolCall, ToolCallAccumulator,
};

use crate::context::{
    self,
    budget::{BudgetMeter, Pressure},
    compact,
    notes::NotesStore,
};
use crate::session::{SessionEvent, SessionWriter};
use crate::tools::{ToolCtx, ToolRegistry};

use super::LoopConfig;
use super::events::{Command, Event};
use super::execute::{ExecutionsOutcome, execute_calls};
use super::side_requests::{run_review, summarize_segment};
use super::state::LoopState;
use super::stream::{StreamOutcome, consume_stream};
use super::system_prompt::l0_message;

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
    client: &dyn ChatProvider,
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
        // Guarded runs pin the accepted work order just above the
        // conversation, so the declared scope is always in view.
        if let Some(digest) = state.work_order_digest() {
            request_messages.push(ChatMessage::system(digest));
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
        if let Ok(mut slot) = state.last_prompt.lock() {
            *slot = Some(
                super::prompt_inspect::PromptInspect::from_request(&request, true).with_manifest(
                    state.active_work_order().as_deref(),
                    u64::from(cfg.max_context_tokens),
                ),
            );
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
                        function: z_engine_provider::FunctionCall {
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

/// Compaction driver (spec section 6): elide L4, summarize L3 into L1.
async fn compact_working_set(
    client: &dyn ChatProvider,
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
