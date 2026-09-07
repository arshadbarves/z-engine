//! Stopping a verification, and saying so honestly.
//!
//! Kept apart from [`super::completion`] because granting completion and
//! giving up on it are different jobs with different failure modes. The
//! rule this file exists to enforce: an interrupted verification is
//! *awaited*, not dropped. Dropping the runner future would kill the
//! child this task spawned and leave its process group unreaped, and
//! would leave no record of how far the checks got — so the stop is
//! bounded, and what it stopped is written down.

use tokio::sync::mpsc::UnboundedReceiver;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::governance::{
    CheckOutcome, CheckStatus, Verification, VerificationManifest, VerificationPlan,
};

use super::events::Command;

/// How long an aborted verification is given to unwind. The runner
/// notices the flag within one poll interval and then kills and reaps the
/// process group it spawned, so this only has to cover the pipe drain —
/// but it is bounded, because a stop that hangs is not a stop.
pub(super) const ABORT_UNWIND_GRACE: std::time::Duration = std::time::Duration::from_secs(15);

/// Let an aborted verification finish stopping, and report what it
/// reached. A runner that will not unwind in time is recorded as such
/// rather than waited on forever.
pub(super) async fn unwind(
    running: std::pin::Pin<&mut impl std::future::Future<Output = Verification>>,
    plan: &VerificationPlan,
) -> Verification {
    match tokio::time::timeout(ABORT_UNWIND_GRACE, running).await {
        Ok(verification) => verification,
        Err(_) => Verification {
            manifest: stopped_manifest(plan, "the checks did not stop within the grace period"),
            // A verification that would not stop cannot say what state it
            // left the workspace in, so it settles nothing: the next turn
            // still owes everything this one did.
            settled: None,
            harness_writes: Vec::new(),
        },
    }
}

/// A manifest for a verification that never reached a verdict. Its one
/// required check is `Unavailable`, so it can only read as blocked — an
/// interrupted run must never look verified.
fn stopped_manifest(plan: &VerificationPlan, detail: &str) -> VerificationManifest {
    VerificationManifest {
        work_order_id: plan.work_order_id.clone(),
        goal: plan.goal.clone(),
        scope: plan.scope.clone(),
        mutated: plan.mutated_paths(),
        breaches: Vec::new(),
        checks: vec![CheckOutcome {
            name: "verification".into(),
            command: "(stopped)".into(),
            required: true,
            status: CheckStatus::Unavailable {
                reason: format!("verification was stopped before it finished: {detail}"),
            },
            duration_ms: 0,
            output_tail: String::new(),
        }],
    }
}

/// Why the checks were stopped. A shutdown ends the run, an abort ends
/// only this turn, and the difference decides whether the stop flag is
/// cleared afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop {
    Abort,
    Shutdown,
}

/// Resolve once the user asks to stop, setting the shared flag so the
/// running checks reap their own process trees. Pends forever otherwise,
/// so it only ever loses the `select!`.
pub(super) async fn watch_for_abort(
    cmd_rx: &mut UnboundedReceiver<Command>,
    abort: &AtomicBool,
) -> Stop {
    while let Some(cmd) = cmd_rx.recv().await {
        let stop = match cmd {
            Command::Abort => Stop::Abort,
            Command::Shutdown => Stop::Shutdown,
            _ => continue,
        };
        abort.store(true, Ordering::Relaxed);
        return stop;
    }
    // The sender is gone: nobody can abort, so never resolve.
    std::future::pending().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> VerificationPlan {
        VerificationPlan {
            work_order_id: "wo-1".into(),
            goal: "g".into(),
            scope: vec![std::path::PathBuf::from("src/lib.rs")],
            mutated: Vec::new(),
            changes: Vec::new(),
            workspace: crate::governance::WorkspaceSnapshot::default(),
            witnesses: Vec::new(),
            acceptance: Vec::new(),
        }
    }

    /// The runner is awaited, not dropped, so the process group it spawned
    /// is killed and reaped before the turn ends.
    #[tokio::test]
    async fn a_runner_that_stops_in_time_reports_what_it_reached() {
        let running = async {
            Verification {
                manifest: stopped_manifest(&plan(), "the checks noticed the stop"),
                settled: None,
                harness_writes: Vec::new(),
            }
        };
        let out = unwind(std::pin::pin!(running), &plan()).await;
        assert!(matches!(
            out.manifest.checks[0].status,
            CheckStatus::Unavailable { .. }
        ));
    }

    /// A verification that had to be given up on must not hand the next
    /// turn a settled workspace: nothing about it was judged.
    #[tokio::test]
    async fn a_runner_that_will_not_stop_settles_nothing() {
        let never = std::future::pending::<Verification>();
        tokio::time::pause();
        let out = tokio::spawn(async move { unwind(std::pin::pin!(never), &plan()).await });
        tokio::time::advance(ABORT_UNWIND_GRACE * 2).await;
        let out = out.await.unwrap();
        assert!(out.settled.is_none() && out.harness_writes.is_empty());
    }

    /// What a runner that will not unwind leaves behind: a manifest that
    /// names the run, proves nothing, and can only read as blocked.
    #[test]
    fn a_stopped_verification_can_never_read_as_verified() {
        let out = stopped_manifest(&plan(), "the checks did not stop in time");
        assert!(matches!(
            out.verdict(),
            crate::governance::Verdict::Blocked(_)
        ));
        assert_eq!(out.work_order_id, "wo-1");
        assert!(
            out.checks[0].required,
            "an absent check must not be optional"
        );
    }

    #[tokio::test]
    async fn an_abort_and_a_shutdown_are_told_apart() {
        for (command, expected) in [
            (Command::Abort, Stop::Abort),
            (Command::Shutdown, Stop::Shutdown),
        ] {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tx.send(command).unwrap();
            let flag = AtomicBool::new(false);
            assert_eq!(watch_for_abort(&mut rx, &flag).await, expected);
            assert!(
                flag.load(Ordering::Relaxed),
                "the checks must be told to stop"
            );
        }
    }
}
