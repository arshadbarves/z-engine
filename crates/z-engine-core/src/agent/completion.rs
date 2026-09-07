//! Who gets to say a guarded turn is finished.
//!
//! A model's final message is a *claim*. In a guarded run that changed
//! the workspace, the claim is worth nothing on its own: this module
//! re-checks the run against the order it declared —
//! [`crate::governance::VerificationRunner`] audits scope, compiles the
//! workspace, and runs the declared acceptance commands — and only a
//! complete, all-passing [`crate::governance::VerificationManifest`]
//! turns that claim into [`TurnOutcome::Completed`].
//!
//! Nothing here decides *what* counts as proof; that is the manifest's
//! verdict. This is the seam that gathers the run's facts, persists the
//! manifest beside the evidence that produced it, and reports the
//! refusal in the vocabulary the UI already speaks.
//!
//! Unguarded runs never reach past the first line: they complete on the
//! model's word exactly as they did before governance existed.

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use std::sync::Arc;
use std::sync::atomic::Ordering;

use z_engine_provider::ChatMessage;

use crate::governance::{Verdict, VerificationRunner, write_manifest};
use crate::tools::ToolCtx;

use super::events::{Command, Event};
use super::state::LoopState;
use super::stop_watch::{Stop, unwind, watch_for_abort};
use super::turn::TurnOutcome;

/// The gate this module speaks for, named in [`Event::TurnBlocked`].
const GATE: &str = "completion";

/// Decide whether the final answer just produced may end the turn.
///
/// `cmd_rx` is drained while the checks run: verification is the last
/// thing a turn does, so nothing else is listening for the user's abort,
/// and without this a stop request would wait out every check's timeout.
pub(super) async fn settle_completion(
    ctx: &ToolCtx,
    state: &mut LoopState,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
) -> TurnOutcome {
    // A run with nothing to account for — unguarded, or guarded and
    // genuinely still — has nothing to prove. A run that cannot say what
    // it did is a different thing entirely, and blocks.
    let plan = match ctx.verification_plan() {
        Ok(Some(plan)) => plan,
        Ok(None) => return TurnOutcome::Completed,
        Err(e) => {
            return TurnOutcome::Blocked {
                gate: GATE,
                reason: e.to_string(),
                manifest_path: None,
            };
        }
    };

    let _ = ev_tx.send(Event::StatusNote(format!(
        "verifying work order {} before completing the turn",
        plan.work_order_id
    )));
    let runner = VerificationRunner::new(&ctx.project_root).with_abort(Arc::clone(&ctx.abort));
    // Pinned rather than dropped on abort: dropping the future would kill
    // the child this task spawned but leave its process group unreaped,
    // so the stop is awaited — bounded — and what it stopped is recorded.
    let mut running = std::pin::pin!(runner.run(&plan));
    let (verification, stop) = tokio::select! {
        done = &mut running => (done, None),
        stop = watch_for_abort(cmd_rx, &ctx.abort) => (unwind(running, &plan).await, Some(stop)),
    };
    let manifest = &verification.manifest;
    let verdict = manifest.verdict();

    // Settle before returning, on every path out of here. A turn that
    // verified hands the next one the workspace its checks produced; a
    // turn that did not keeps owing what it changed, and only the
    // harness's own writes are absorbed.
    ctx.settle_turn(
        stop.is_none() && matches!(verdict, Verdict::Complete),
        &verification,
    );

    // Persist first: the refusal points at the manifest, and a manifest
    // that could not be written is itself reported rather than ignored.
    let manifest_path = match state.run_dir.as_ref().map(|d| write_manifest(d, manifest)) {
        Some(Ok(path)) => Some(path.display().to_string()),
        Some(Err(e)) => {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "verification manifest could not be written: {e}"
            )));
            None
        }
        None => None,
    };

    if let Some(stop) = stop {
        // The checks are no longer running, so the flag has done its job.
        // Leaving it set would make every later tool call in this run
        // believe the user is still asking to stop; a shutdown keeps it,
        // because there is no later turn to protect.
        if matches!(stop, Stop::Abort) {
            ctx.abort.store(false, Ordering::Relaxed);
        }
        let _ = ev_tx.send(Event::StatusNote(match &manifest_path {
            Some(path) => format!("verification stopped; what it reached is recorded in {path}"),
            None => "verification stopped before it finished".into(),
        }));
        return TurnOutcome::Aborted;
    }

    // The transcript keeps the evidence either way, so a follow-up turn
    // starts from what actually ran rather than from the model's belief.
    state.working.push(ChatMessage::user(format!(
        "[harness verification]\n{}",
        manifest.summary()
    )));

    match verdict {
        Verdict::Complete => TurnOutcome::Completed,
        Verdict::Blocked(reason) => TurnOutcome::Blocked {
            gate: GATE,
            reason,
            manifest_path,
        },
    }
}

#[cfg(test)]
mod tests;
