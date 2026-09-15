use z_engine_context::{
    CheckObservation, CheckOutcome, EvidenceArtifact, Requirement, TaskObservation, TaskStatus,
};

pub fn task() -> TaskObservation {
    TaskObservation {
        report_schema_version: 1,
        task_id: "task-7".into(),
        goal: "Implement the requested parser without changing its API.".into(),
        workspace_root: "/fixture/not-a-real-repository".into(),
        status: TaskStatus::NeedsVerification,
        requirements: vec![Requirement {
            id: "requirement-1".into(),
            description: "Preserve the public API.".into(),
        }],
        blockers: Vec::new(),
        supervision: None,
        checks: Vec::new(),
        changed_paths: Vec::new(),
    }
}

pub fn check(id: &str, outcome: CheckOutcome) -> CheckObservation {
    CheckObservation {
        id: id.into(),
        outcome,
        input_fingerprint: Some("source-version-3".into()),
        started_at_ms: 123,
        duration_ms: 45,
        exit_code: Some(if outcome == CheckOutcome::Passed {
            0
        } else {
            1
        }),
        tests_run: Some(9),
        command: vec!["cargo".into(), "test".into(), "--workspace".into()],
        cwd: "/fixture/not-a-real-repository".into(),
        toolchain: "recorded-toolchain".into(),
        summary: format!("observed {outcome:?}"),
        stdout: Some(EvidenceArtifact {
            path: "/fixture/check.stdout".into(),
            digest: "recorded-artifact-digest".into(),
        }),
        stderr: None,
    }
}
