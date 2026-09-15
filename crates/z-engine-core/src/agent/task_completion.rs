//! Durable task projections and the completion boundary, independent of model prose.

use std::path::PathBuf;

use tokio::sync::mpsc::UnboundedSender;

use crate::session::{SessionEvent, SessionWriter};
use crate::tools::ToolCtx;
use crate::verification::{
    Requirement, TaskReport, TaskStatus, VerificationError, WorkspaceSnapshot,
};

use super::events::Event;

#[cfg(test)]
#[path = "task_completion_tests.rs"]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(super) enum TaskLifecycleError {
    #[error("task state lock poisoned")]
    State,
    #[error("task has not been initialized")]
    Missing,
    #[error("could not persist task evidence: {0}")]
    Storage(#[from] std::io::Error),
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("workspace observation failed: {0}")]
    Worker(#[from] tokio::task::JoinError),
}

async fn snapshot(root: PathBuf) -> Result<WorkspaceSnapshot, TaskLifecycleError> {
    Ok(tokio::task::spawn_blocking(move || WorkspaceSnapshot::capture(&root)).await??)
}

pub(super) async fn begin(
    ctx: &ToolCtx,
    goal: &str,
    recorder: &mut Option<SessionWriter>,
    events: &UnboundedSender<Event>,
) -> Result<Option<WorkspaceSnapshot>, TaskLifecycleError> {
    let mut report = TaskReport {
        schema_version: 1,
        task_id: ulid::Ulid::new().to_string(),
        goal: goal.to_string(),
        workspace_root: ctx.project_root.to_string_lossy().into_owned(),
        status: TaskStatus::Running,
        requirements: vec![Requirement {
            id: "goal".into(),
            description: goal.to_string(),
        }],
        checks: Vec::new(),
        assessment: None,
        blockers: Vec::new(),
        changed_paths: Vec::new(),
        supervision: None,
    };
    if recorder.is_none() {
        report
            .blockers
            .push("No durable session recorder is available.".into());
    }
    if !ctx.project_root.join("Cargo.toml").is_file() {
        report.status = TaskStatus::Unassessed;
        report
            .blockers
            .push("S1 completion verification supports Rust Cargo workspaces only.".into());
    }
    let baseline = match snapshot(ctx.project_root.clone()).await {
        Ok(value) => Some(value),
        Err(error) => {
            report.status = TaskStatus::Unassessed;
            report.blockers.push(error.to_string());
            None
        }
    };
    *ctx.task.lock().map_err(|_| TaskLifecycleError::State)? = Some(report);
    publish(ctx, recorder, events)?;
    Ok(baseline)
}

pub(super) fn block(ctx: &ToolCtx, reason: String) -> Result<(), TaskLifecycleError> {
    let mut guard = ctx.task.lock().map_err(|_| TaskLifecycleError::State)?;
    if let Some(report) = guard.as_mut() {
        if !report.blockers.contains(&reason) {
            report.blockers.push(reason);
        }
        report.status = TaskStatus::Blocked;
        report.assessment = None;
    }
    Ok(())
}

pub(super) fn invalidate(ctx: &ToolCtx) -> Result<(), TaskLifecycleError> {
    let mut guard = ctx.task.lock().map_err(|_| TaskLifecycleError::State)?;
    if let Some(report) = guard.as_mut() {
        report.assessment = None;
        report.status = TaskStatus::NeedsVerification;
        for check in &mut report.checks {
            if check.outcome == crate::verification::CheckOutcome::Passed {
                check.outcome = crate::verification::CheckOutcome::Stale;
            }
        }
    }
    Ok(())
}

pub(super) fn invalidate_and_publish(
    ctx: &ToolCtx,
    recorder: &mut Option<SessionWriter>,
    events: &UnboundedSender<Event>,
) -> Result<(), TaskLifecycleError> {
    let present = ctx
        .task
        .lock()
        .map_err(|_| TaskLifecycleError::State)?
        .is_some();
    if present {
        invalidate(ctx)?;
        publish(ctx, recorder, events)?;
    }
    Ok(())
}

/// Reports are published only after their evidence references and state are durable.
pub(super) fn publish(
    ctx: &ToolCtx,
    recorder: &mut Option<SessionWriter>,
    events: &UnboundedSender<Event>,
) -> Result<(), TaskLifecycleError> {
    persist_projection(ctx, recorder, events, false)
}

fn persist_projection(
    ctx: &ToolCtx,
    recorder: &mut Option<SessionWriter>,
    events: &UnboundedSender<Event>,
    assessed: bool,
) -> Result<(), TaskLifecycleError> {
    let mut report = ctx
        .task
        .lock()
        .map_err(|_| TaskLifecycleError::State)?
        .clone()
        .ok_or(TaskLifecycleError::Missing)?;
    if report.status == TaskStatus::Complete && (!assessed || recorder.is_none()) {
        report.status = TaskStatus::Unassessed;
        report
            .blockers
            .push("Completion requires the runtime assessment gate and durable storage.".into());
        *ctx.task.lock().map_err(|_| TaskLifecycleError::State)? = Some(report.clone());
    }
    if assessed && ctx.aborted() {
        report.status = TaskStatus::Stopped;
        if let Some(supervision) = &mut report.supervision {
            supervision.last_action = z_engine_runtime::SupervisionAction::Stopped;
            supervision.reason = "The user cancelled before completion was committed.".into();
        }
        *ctx.task.lock().map_err(|_| TaskLifecycleError::State)? = Some(report.clone());
    }
    if let Some(writer) = recorder.as_mut() {
        if let Err(error) = writer.record_durable(&SessionEvent::TaskUpdated {
            report: Box::new(report.clone()),
        }) {
            report.status = TaskStatus::Blocked;
            report
                .blockers
                .push(format!("Task evidence could not be persisted: {error}"));
            if let Some(supervision) = &mut report.supervision {
                supervision.last_action = z_engine_runtime::SupervisionAction::Blocked;
                supervision.reason = "Task evidence could not be persisted.".into();
            }
            *ctx.task.lock().map_err(|_| TaskLifecycleError::State)? = Some(report.clone());
            let _ = events.send(Event::TaskUpdated { report });
            return Err(error.into());
        }
    }
    let _ = events.send(Event::TaskUpdated { report });
    Ok(())
}

pub(super) async fn finish(
    ctx: &ToolCtx,
    baseline: Option<&WorkspaceSnapshot>,
    terminal: Option<TaskStatus>,
    recorder: &mut Option<SessionWriter>,
    events: &UnboundedSender<Event>,
) -> Result<TaskStatus, TaskLifecycleError> {
    let (mut report, _) = assess_current(ctx, baseline, terminal, events).await?;
    if let Some(supervision) = &mut report.supervision {
        use z_engine_runtime::SupervisionAction;
        let terminal_action = match report.status {
            TaskStatus::Complete => Some((
                SupervisionAction::Complete,
                "Completion verified and ready for durable recording.",
            )),
            TaskStatus::Stopped => {
                Some((SupervisionAction::Stopped, "The user cancelled the task."))
            }
            TaskStatus::Interrupted => Some((
                SupervisionAction::Interrupted,
                "Execution failed; the task remains incomplete.",
            )),
            TaskStatus::Blocked if supervision.last_action != SupervisionAction::Blocked => Some((
                SupervisionAction::Blocked,
                "Final assessment is blocked; inspect the recorded blockers.",
            )),
            _ => None,
        };
        if let Some((action, reason)) = terminal_action {
            supervision.last_action = action;
            supervision.reason = reason.into();
        }
    }
    *ctx.task.lock().map_err(|_| TaskLifecycleError::State)? = Some(report);
    persist_projection(ctx, recorder, events, true)?;
    Ok(ctx
        .task
        .lock()
        .map_err(|_| TaskLifecycleError::State)?
        .as_ref()
        .ok_or(TaskLifecycleError::Missing)?
        .status)
}

/// Evaluate without publishing a Complete projection before the final commit.
pub(super) async fn assess_current(
    ctx: &ToolCtx,
    baseline: Option<&WorkspaceSnapshot>,
    terminal: Option<TaskStatus>,
    events: &UnboundedSender<Event>,
) -> Result<(TaskReport, Option<String>), TaskLifecycleError> {
    let mut report = ctx
        .task
        .lock()
        .map_err(|_| TaskLifecycleError::State)?
        .clone()
        .ok_or(TaskLifecycleError::Missing)?;
    if terminal.is_none() {
        let _ = events.send(Event::StatusNote("Assessing task completion".into()));
    }
    let current = snapshot(ctx.project_root.clone()).await;
    let fingerprint = current.as_ref().ok().map(|value| value.fingerprint.clone());
    let terminal = if ctx.aborted() {
        Some(TaskStatus::Stopped)
    } else {
        terminal
    };
    match current {
        Ok(current) => {
            if let Some(baseline) = baseline {
                report.changed_paths = baseline.changed_paths(&current);
            }
            if let Some(status) = terminal {
                report.status = status;
            } else {
                // The model supplies coverage; this gate checks actual evidence and versions.
                report = tokio::task::spawn_blocking(move || {
                    if let Err(error) = crate::verification::assess(&mut report, &current) {
                        report.status = TaskStatus::Blocked;
                        report.blockers.push(error.to_string());
                    }
                    report
                })
                .await?;
            }
        }
        Err(error) => {
            report.status = terminal.unwrap_or(TaskStatus::Blocked);
            report.blockers.push(error.to_string());
        }
    }
    if ctx.aborted() {
        report.status = TaskStatus::Stopped;
    }
    Ok((report, fingerprint))
}
