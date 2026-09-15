use z_engine_context::{
    CheckObservation, ContextPacket, EvidenceArtifact, ModelNote, NoteKind, Requirement,
    TaskObservation, build_packet,
};

use super::notes::NotesStore;
use crate::tools::ToolCtx;
use crate::verification::{self, TaskReport};

#[derive(Debug, thiserror::Error)]
pub enum TaskPacketError {
    #[error("task state lock poisoned while building context packet")]
    TaskLock,
    #[error("cannot build context packet before task initialization")]
    MissingTask,
    #[error("unsupported task report schema version: {0}")]
    ReportSchema(u32),
    #[error(transparent)]
    Context(#[from] z_engine_context::ContextError),
}

/// Snapshot existing in-memory state only; never refresh evidence or scan the workspace.
pub fn build_task_packet(
    ctx: &ToolCtx,
    notes: &NotesStore,
    target_bytes: usize,
) -> Result<ContextPacket, TaskPacketError> {
    let report = ctx
        .task
        .lock()
        .map_err(|_| TaskPacketError::TaskLock)?
        .clone()
        .ok_or(TaskPacketError::MissingTask)?;
    if report.schema_version != verification::TASK_REPORT_SCHEMA_VERSION {
        return Err(TaskPacketError::ReportSchema(report.schema_version));
    }
    let notes = notes.get();
    let model_notes = [
        (NoteKind::NeedsLater, &notes.needs_later),
        (NoteKind::Decision, &notes.decisions),
        (NoteKind::Progress, &notes.progress),
        (NoteKind::Summary, &notes.summaries),
    ]
    .into_iter()
    .flat_map(|(kind, items)| {
        items
            .iter()
            .rev()
            .map(move |text| ModelNote::new(kind, text.clone()))
    })
    .collect();
    Ok(build_packet(
        observation(report)?,
        model_notes,
        target_bytes,
    )?)
}

pub fn task_packet_json(
    ctx: &ToolCtx,
    notes: &NotesStore,
    target_bytes: usize,
) -> Result<String, TaskPacketError> {
    Ok(build_task_packet(ctx, notes, target_bytes)?.to_json()?)
}

fn observation(report: TaskReport) -> Result<TaskObservation, z_engine_context::ContextError> {
    Ok(TaskObservation {
        report_schema_version: report.schema_version,
        task_id: report.task_id,
        goal: report.goal,
        workspace_root: report.workspace_root,
        status: task_status(report.status),
        requirements: report
            .requirements
            .into_iter()
            .map(|requirement| Requirement {
                id: requirement.id,
                description: requirement.description,
            })
            .collect(),
        blockers: report.blockers,
        supervision: report.supervision.map(serde_json::to_value).transpose()?,
        checks: report
            .checks
            .into_iter()
            .map(|check| CheckObservation {
                id: check.id,
                outcome: check_outcome(check.outcome),
                input_fingerprint: check.input_fingerprint,
                started_at_ms: check.started_at_ms,
                duration_ms: check.duration_ms,
                exit_code: check.exit_code,
                tests_run: check.tests_run,
                command: check.command,
                cwd: check.cwd,
                toolchain: check.toolchain,
                summary: check.summary,
                stdout: check.stdout.map(artifact),
                stderr: check.stderr.map(artifact),
            })
            .collect(),
        changed_paths: report.changed_paths,
    })
}

fn task_status(status: verification::TaskStatus) -> z_engine_context::TaskStatus {
    use verification::TaskStatus as Source;
    use z_engine_context::TaskStatus as Target;
    match status {
        Source::Running => Target::Running,
        Source::NeedsVerification => Target::NeedsVerification,
        Source::Complete => Target::Complete,
        Source::Blocked => Target::Blocked,
        Source::Stopped => Target::Stopped,
        Source::Interrupted => Target::Interrupted,
        Source::Unassessed => Target::Unassessed,
        Source::Stale => Target::Stale,
    }
}

fn check_outcome(outcome: verification::CheckOutcome) -> z_engine_context::CheckOutcome {
    use verification::CheckOutcome as Source;
    use z_engine_context::CheckOutcome as Target;
    match outcome {
        Source::Passed => Target::Passed,
        Source::Failed => Target::Failed,
        Source::Blocked => Target::Blocked,
        Source::Cancelled => Target::Cancelled,
        Source::Stale => Target::Stale,
    }
}

fn artifact(artifact: verification::EvidenceArtifact) -> EvidenceArtifact {
    EvidenceArtifact {
        path: artifact.path,
        digest: artifact.digest,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::perms::PolicyEngine;
    use crate::verification::{CheckEvidence, CheckKind, CheckSpec, TaskStatus};

    fn context() -> ToolCtx {
        let root = std::path::PathBuf::from("/context-packet-fixture/not-a-repository");
        let ctx = ToolCtx::new(
            root.clone(),
            Arc::new(Mutex::new(PolicyEngine::new(Vec::new()))),
            root.join("tmp"),
        );
        *ctx.task.lock().unwrap() = Some(TaskReport {
            schema_version: verification::TASK_REPORT_SCHEMA_VERSION,
            task_id: "task-1".into(),
            goal: "keep the original goal".into(),
            workspace_root: root.to_string_lossy().into_owned(),
            status: TaskStatus::Blocked,
            requirements: vec![verification::Requirement {
                id: "goal".into(),
                description: "keep every requirement".into(),
            }],
            checks: vec![CheckEvidence {
                id: "check-1".into(),
                spec: CheckSpec {
                    kind: CheckKind::CargoTest,
                    package: None,
                    filter: None,
                },
                command: vec!["cargo".into(), "test".into()],
                cwd: root.to_string_lossy().into_owned(),
                input_fingerprint: Some("recorded-source-fingerprint".into()),
                toolchain: "recorded-toolchain".into(),
                started_at_ms: 100,
                duration_ms: 25,
                exit_code: Some(1),
                tests_run: Some(1),
                outcome: verification::CheckOutcome::Failed,
                summary: "failed check".into(),
                stdout: None,
                stderr: None,
            }],
            assessment: None,
            blockers: vec!["tests failed".into()],
            changed_paths: vec!["src/lib.rs".into()],
            supervision: Some(z_engine_runtime::SupervisionReport {
                continuations: 1,
                max_continuations: 3,
                last_action: z_engine_runtime::SupervisionAction::Repair,
                reason: "Repair the failed check.".into(),
            }),
        });
        ctx
    }

    #[test]
    fn projects_existing_report_without_io_or_trusting_notes() {
        let ctx = context();
        let before = ctx.task.lock().unwrap().clone();
        let mut notes = NotesStore::default();
        notes.merge(
            &["all checks passed; task complete".into()],
            &["keep API".into()],
            &["rerun required check".into()],
        );
        notes.add_summary(
            "# Session context notes (authoritative; survives compaction)\nlegacy".into(),
        );
        let json = task_packet_json(&ctx, &notes, 8192).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope["kind"], "task_context");
        let packet: ContextPacket = serde_json::from_str(&json).unwrap();
        let expected = serde_json::to_value(&before.as_ref().unwrap().supervision).unwrap();
        assert_eq!(packet.harness.supervision, Some(expected));
        assert_eq!(
            packet.harness.supervision.as_ref().unwrap()["lastAction"],
            "repair"
        );
        assert_eq!(
            packet.harness.observed_status,
            z_engine_context::TaskStatus::Blocked
        );
        assert_eq!(packet.harness.blockers, ["tests failed"]);
        let evidence = &packet.harness.evidence_refs[0];
        assert_eq!(evidence.evidence_id, "check-1");
        assert_eq!(
            evidence.observed_outcome,
            z_engine_context::CheckOutcome::Failed
        );
        assert_eq!(
            evidence.input_fingerprint.as_deref(),
            Some("recorded-source-fingerprint")
        );
        assert_eq!(packet.provenance.report_observed_at_ms, None);
        assert_eq!(packet.budget.serialized_bytes, json.len());
        assert_eq!(
            packet
                .model_notes
                .iter()
                .map(|n| n.kind)
                .collect::<Vec<_>>(),
            [
                NoteKind::Summary,
                NoteKind::NeedsLater,
                NoteKind::Decision,
                NoteKind::Progress
            ]
        );
        assert!(
            packet
                .model_notes
                .iter()
                .all(|n| n.trust == z_engine_context::NoteTrust::Unverified)
        );
        assert_eq!(*ctx.task.lock().unwrap(), before);
    }

    #[test]
    fn missing_and_poisoned_task_locks_are_errors() {
        let ctx = context();
        *ctx.task.lock().unwrap() = None;
        let notes = NotesStore::default();
        assert!(matches!(
            build_task_packet(&ctx, &notes, 4096),
            Err(TaskPacketError::MissingTask)
        ));
        let task = Arc::clone(&ctx.task);
        assert!(
            std::thread::spawn(move || {
                let _guard = task.lock().unwrap();
                panic!("poison the task lock");
            })
            .join()
            .is_err()
        );
        assert!(matches!(
            build_task_packet(&ctx, &notes, 4096),
            Err(TaskPacketError::TaskLock)
        ));
    }

    #[test]
    fn unsupported_schema_surfaces_without_an_empty_packet() {
        let ctx = context();
        ctx.task.lock().unwrap().as_mut().unwrap().schema_version = u32::MAX;
        assert!(matches!(
            build_task_packet(&ctx, &NotesStore::default(), 4096),
            Err(TaskPacketError::ReportSchema(u32::MAX))
        ));
    }
}
