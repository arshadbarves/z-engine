use std::sync::{Arc, Mutex};

use crate::perms::PolicyEngine;
use crate::tools::ToolCtx;
use crate::verification::{CompletionAssessment, TaskReport, TaskStatus};

use super::*;

fn context(root: &std::path::Path) -> ToolCtx {
    let ctx = ToolCtx::new(
        root.to_path_buf(),
        Arc::new(Mutex::new(PolicyEngine::new(Vec::new()))),
        root.join("tmp"),
    );
    *ctx.task.lock().unwrap() = Some(TaskReport {
        schema_version: 1,
        task_id: "task-1".into(),
        goal: "original goal".into(),
        workspace_root: root.to_string_lossy().into_owned(),
        status: TaskStatus::Complete,
        requirements: vec![crate::verification::Requirement {
            id: "goal".into(),
            description: "original goal".into(),
        }],
        checks: Vec::new(),
        assessment: Some(CompletionAssessment {
            summary: "proposal".into(),
            coverage: Vec::new(),
        }),
        blockers: Vec::new(),
        changed_paths: Vec::new(),
        supervision: None,
    });
    ctx
}

#[test]
fn completion_without_a_durable_recorder_is_never_published() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    publish(&ctx, &mut None, &sender).unwrap();
    let Event::TaskUpdated { report } = receiver.try_recv().unwrap() else {
        panic!("task event")
    };
    assert_eq!(report.status, TaskStatus::Unassessed);
    assert!(
        report
            .blockers
            .iter()
            .any(|reason| reason.contains("durable"))
    );
}

#[test]
fn interim_projection_cannot_certify_a_model_claim() {
    let root = tempfile::tempdir().unwrap();
    let sessions = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let mut recorder = Some(SessionWriter::create(sessions.path()).unwrap());
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    publish(&ctx, &mut recorder, &sender).unwrap();
    let Event::TaskUpdated { report } = receiver.try_recv().unwrap() else {
        panic!("task event")
    };
    assert_eq!(report.status, TaskStatus::Unassessed);
}

#[test]
fn failed_durable_write_never_publishes_complete() {
    let root = tempfile::tempdir().unwrap();
    let sessions = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let writer = SessionWriter::create(sessions.path()).unwrap();
    let readonly = std::fs::File::open(&writer.path).unwrap();
    let mut recorder = Some(SessionWriter::from_file_for_test(
        readonly,
        writer.path.clone(),
    ));
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    assert!(persist_projection(&ctx, &mut recorder, &sender, true).is_err());
    let Event::TaskUpdated { report } = receiver.try_recv().unwrap() else {
        panic!("task event")
    };
    assert_eq!(report.status, TaskStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|reason| reason.contains("persisted"))
    );
    assert!(receiver.try_recv().is_err());
    assert!(crate::session::read_events(&writer.path).is_err());
}

#[test]
fn cancellation_at_the_commit_boundary_publishes_stopped() {
    let root = tempfile::tempdir().unwrap();
    let sessions = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let mut recorder = Some(SessionWriter::create(sessions.path()).unwrap());
    ctx.abort.store(true, std::sync::atomic::Ordering::Relaxed);
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    persist_projection(&ctx, &mut recorder, &sender, true).unwrap();
    let Event::TaskUpdated { report } = receiver.try_recv().unwrap() else {
        panic!("task event")
    };
    assert_eq!(report.status, TaskStatus::Stopped);
}

#[test]
fn managed_edits_invalidate_coverage_and_keep_the_original_goal() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    std::fs::write(root.path().join("src.rs"), "old").unwrap();
    crate::agent::operation_tracking::before_call(
        "edit_file",
        &serde_json::json!({"path":"src.rs"}),
        &ctx,
    )
    .unwrap();
    let guard = ctx.task.lock().unwrap();
    let report = guard.as_ref().unwrap();
    assert_eq!(report.status, TaskStatus::NeedsVerification);
    assert!(report.assessment.is_none());
    assert_eq!(report.requirements[0].description, "original goal");
}

#[test]
fn unknown_effects_and_denied_checks_are_explicit_blockers() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    crate::agent::operation_tracking::before_call(
        "bash",
        &serde_json::json!({"command":"custom-build-script"}),
        &ctx,
    )
    .unwrap();
    crate::agent::operation_tracking::denied("run_verification", &ctx).unwrap();
    let guard = ctx.task.lock().unwrap();
    let report = guard.as_ref().unwrap();
    assert_eq!(report.status, TaskStatus::Blocked);
    assert_eq!(report.blockers.len(), 2);
    assert!(report.assessment.is_none());
}

#[test]
fn read_only_commands_do_not_invalidate_evidence() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    crate::agent::operation_tracking::before_call(
        "bash",
        &serde_json::json!({"command":"pwd"}),
        &ctx,
    )
    .unwrap();
    assert_eq!(
        ctx.task.lock().unwrap().as_ref().unwrap().status,
        TaskStatus::Complete
    );
}
