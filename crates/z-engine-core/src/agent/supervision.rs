//! Response boundaries can request another attempt, but never bypass evidence,
//! permission, cancellation, or durable-recording requirements.

use tokio::sync::mpsc::UnboundedSender;
use z_engine_runtime::{Boundary, SupervisionAction, Supervisor, WorkAction};

use crate::session::SessionWriter;
use crate::tools::ToolCtx;
use crate::verification::{CheckOutcome, TaskReport, TaskStatus, WorkspaceSnapshot};

use super::events::Event;
use super::task_completion::{self, TaskLifecycleError};

pub(super) async fn at_boundary(
    supervisor: &mut Supervisor,
    ctx: &ToolCtx,
    baseline: Option<&WorkspaceSnapshot>,
    recorder: &mut Option<SessionWriter>,
    events: &UnboundedSender<Event>,
) -> Result<bool, TaskLifecycleError> {
    let (candidate, fingerprint) =
        task_completion::assess_current(ctx, baseline, None, events).await?;
    let decision = supervisor.decide(boundary(&candidate, fingerprint.as_deref(), ctx.aborted()));
    let continues = decision.last_action.continues();
    {
        let mut guard = ctx.task.lock().map_err(|_| TaskLifecycleError::State)?;
        let report = guard.as_mut().ok_or(TaskLifecycleError::Missing)?;
        report.changed_paths = candidate.changed_paths;
        report.checks = candidate.checks;
        report.assessment = candidate.assessment;
        report.blockers = candidate.blockers;
        if candidate.status != TaskStatus::Complete {
            report.status = candidate.status;
        }
        if decision.last_action == SupervisionAction::Blocked {
            if !report.blockers.contains(&decision.reason) {
                report.blockers.push(decision.reason.clone());
            }
            report.status = TaskStatus::Blocked;
        } else if continues {
            report.status = TaskStatus::Running;
        }
        // Complete remains only a proposal until finish() revalidates and commits.
        report.supervision = Some(decision.clone());
    }
    task_completion::publish(ctx, recorder, events)?;
    if continues {
        let _ = events.send(Event::StatusNote(format!(
            "Continuing task ({}/{}): {}",
            decision.continuations, decision.max_continuations, decision.reason
        )));
    }
    Ok(continues)
}

fn boundary(report: &TaskReport, fingerprint: Option<&str>, cancelled: bool) -> Boundary {
    if cancelled || report.status == TaskStatus::Stopped {
        return Boundary::Cancelled;
    }
    if report.status == TaskStatus::Complete {
        return Boundary::Complete;
    }
    if report.changed_paths.is_empty() && report.checks.is_empty() {
        return Boundary::Ineligible;
    }
    if !report.blockers.is_empty() {
        return Boundary::Blocked(report.blockers.join("\n"));
    }
    let Some(fingerprint) = fingerprint else {
        return Boundary::Blocked("Workspace state cannot be safely observed.".into());
    };
    let fresh_full_pass = report.checks.iter().any(|check| {
        check.spec.is_full_workspace_test()
            && check.outcome == CheckOutcome::Passed
            && check.exit_code == Some(0)
            && check.tests_run.is_some_and(|count| count > 0)
    });
    let next = match report.checks.last().map(|check| check.outcome) {
        Some(CheckOutcome::Blocked | CheckOutcome::Cancelled) => {
            return Boundary::Blocked(
                "The latest check was blocked or cancelled; resolve it before continuing.".into(),
            );
        }
        Some(CheckOutcome::Failed)
            if report
                .checks
                .last()
                .and_then(|check| check.input_fingerprint.as_deref())
                == Some(fingerprint) =>
        {
            WorkAction::Repair
        }
        Some(CheckOutcome::Passed) if fresh_full_pass => WorkAction::Continue,
        _ => WorkAction::Verify,
    };
    // New IDs, repeated identical checks, and model summaries are not progress.
    let latest = report.checks.last().map(|check| {
        serde_json::json!({
            "spec": check.spec,
            "input": check.input_fingerprint,
            "outcome": check.outcome,
            "exit": check.exit_code,
            "tests": check.tests_run,
        })
    });
    Boundary::Incomplete {
        progress_key: serde_json::json!([fingerprint, latest]).to_string(),
        next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> TaskReport {
        TaskReport {
            schema_version: 1,
            task_id: "task".into(),
            goal: "fix".into(),
            workspace_root: "/workspace".into(),
            status: TaskStatus::NeedsVerification,
            requirements: vec![],
            checks: vec![],
            assessment: None,
            blockers: vec![],
            changed_paths: vec!["src/lib.rs".into()],
            supervision: None,
        }
    }

    #[test]
    fn unchanged_information_requests_do_not_loop() {
        let mut value = report();
        value.changed_paths.clear();
        assert_eq!(boundary(&value, Some("v1"), false), Boundary::Ineligible);
    }

    #[test]
    fn cancellation_and_denial_cannot_be_overridden_by_continuation() {
        let mut value = report();
        assert_eq!(boundary(&value, Some("v1"), true), Boundary::Cancelled);
        value.blockers.push("An edit was denied.".into());
        assert!(matches!(
            boundary(&value, Some("v1"), false),
            Boundary::Blocked(_)
        ));
    }

    #[test]
    fn an_edit_without_checks_requires_verification() {
        assert!(matches!(
            boundary(&report(), Some("v1"), false),
            Boundary::Incomplete {
                next: WorkAction::Verify,
                ..
            }
        ));
    }
}
