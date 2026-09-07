//! Execution: run the calls [`super::decide`] cleared — safe tools
//! concurrently, unsafe ones serially — and map results and errors into
//! transcript entries in the model's original call order.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use z_engine_provider::ToolCall;

use crate::replay::ToolDisposition;
use crate::tools::{ToolCtx, ToolError, ToolOutput, ToolRegistry};

use super::decide::{Decided, Decisions, REFUSAL, Verdict, decide_calls};
use super::events::{Command, Event};
use super::state::LoopState;

const INPUT_PREVIEW_CHARS: usize = 160;

pub(super) enum ExecutionsOutcome {
    /// `(tool_call_id, transcript content)` in original call order.
    Ran(Vec<(String, String)>),
    Aborted,
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
    // Phase 1 — decide every call up front (approvals surface sequentially).
    let decided = match decide_calls(
        &calls, registry, ctx, cmd_rx, ev_tx, state, abort_flag, mode,
    )
    .await
    {
        Decisions::Made(decided) => decided,
        Decisions::Aborted => return ExecutionsOutcome::Aborted,
    };

    // Phase 2 — run: concurrency-safe tools together, unsafe ones serially.
    let mut outcomes: HashMap<usize, String> = HashMap::new();
    let mut safe_batch: Vec<(usize, ToolCall)> = Vec::new();
    for (idx, call) in calls.iter().enumerate() {
        if !matches!(decided[idx].verdict, Verdict::Run) {
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
        let futs = safe_batch.iter().map(|(idx, call)| {
            let call = call.clone();
            let sequence = decided[*idx].sequence;
            let ctx = ctx.clone();
            let ev_tx = ev_tx.clone();
            async move { run_one(call, sequence, &ctx, registry, &ev_tx).await }
        });
        let done = futures::future::join_all(futs).await;
        for ((idx, _), content) in safe_batch.iter().zip(done) {
            outcomes.insert(*idx, content);
        }
    }

    for (idx, call) in calls.iter().enumerate() {
        if outcomes.contains_key(&idx) || !matches!(decided[idx].verdict, Verdict::Run) {
            continue;
        }
        let content = run_one(call.clone(), decided[idx].sequence, ctx, registry, ev_tx).await;
        outcomes.insert(idx, content);
    }

    // Phase 3 — transcript entries in original order; refusals become polite
    // declines addressed to the same tool_call_id (spec §5).
    ExecutionsOutcome::Ran(transcript(&calls, &decided, &mut outcomes))
}

fn transcript(
    calls: &[ToolCall],
    decided: &[Decided],
    outcomes: &mut HashMap<usize, String>,
) -> Vec<(String, String)> {
    calls
        .iter()
        .enumerate()
        .map(|(idx, call)| {
            let content = outcomes.remove(&idx).unwrap_or_else(|| {
                debug_assert!(matches!(decided[idx].verdict, Verdict::Refused));
                REFUSAL.to_string()
            });
            (call.id.clone(), content)
        })
        .collect()
}

pub(super) fn parse_input(arguments: &str) -> serde_json::Value {
    serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null)
}

/// Execute one allowed/approved call: events + timing + error mapping.
/// Errors become `"ERROR: …"` transcript text (self-correction path).
async fn run_one(
    call: ToolCall,
    sequence: Option<u64>,
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
        ctx.record_tool_outcome(
            sequence,
            &name,
            ToolDisposition::Abandoned,
            false,
            "[aborted]",
        );
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
            let text = format!("ERROR: {e}");
            ctx.record_tool_outcome(sequence, &name, ToolDisposition::Executed, false, &text);
            let _ = ev_tx.send(Event::ToolCallFinished {
                name,
                ok: false,
                duration_ms,
                summary: e.to_string(),
            });
            return text;
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

    ctx.record_tool_outcome(
        sequence,
        &name,
        ToolDisposition::Executed,
        out.ok,
        &out.result,
    );
    let _ = ev_tx.send(Event::ToolCallFinished {
        name,
        ok: out.ok,
        duration_ms,
        summary: out.summary,
    });
    out.result
}

pub(super) fn input_preview(input: &serde_json::Value) -> String {
    let s = serde_json::to_string(input).unwrap_or_else(|_| "<unserializable>".into());
    let mut s: String = s.chars().take(INPUT_PREVIEW_CHARS).collect();
    if s.chars().count() == INPUT_PREVIEW_CHARS {
        s.push('\u{2026}');
    }
    s
}
