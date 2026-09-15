use super::artifacts::digest_file;
use super::gate::assess_with_identity;
use super::test_workspace::TestWorkspace;
use super::*;

fn fixture() -> (TestWorkspace, WorkspaceSnapshot, TaskReport) {
    let root = TestWorkspace::new();
    let outputs = root.0.join(".z-engine/evidence");
    std::fs::create_dir_all(&outputs).unwrap();
    let out = outputs.join("out.log");
    let err = outputs.join("err.log");
    std::fs::write(&out, "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n").unwrap();
    std::fs::write(&err, "").unwrap();
    let snapshot = WorkspaceSnapshot::capture(root.path()).unwrap();
    let spec = CheckSpec {
        kind: CheckKind::CargoTest,
        package: None,
        filter: None,
    };
    let check = CheckEvidence {
        id: "check-1".into(),
        command: spec.command().unwrap(),
        spec,
        cwd: root.0.to_string_lossy().into_owned(),
        input_fingerprint: Some(snapshot.fingerprint.clone()),
        toolchain: "fixture-toolchain".into(),
        started_at_ms: 1,
        duration_ms: 1,
        exit_code: Some(0),
        tests_run: Some(2),
        outcome: CheckOutcome::Passed,
        summary: "2 passed".into(),
        stdout: Some(EvidenceArtifact {
            digest: digest_file(&out, 1024).unwrap().0,
            path: out.to_string_lossy().into_owned(),
        }),
        stderr: Some(EvidenceArtifact {
            digest: digest_file(&err, 1024).unwrap().0,
            path: err.to_string_lossy().into_owned(),
        }),
    };
    let report = TaskReport {
        schema_version: 1,
        task_id: "task-1".into(),
        goal: "fix".into(),
        workspace_root: root.0.to_string_lossy().into_owned(),
        status: TaskStatus::Running,
        requirements: vec![Requirement {
            id: "goal".into(),
            description: "fix".into(),
        }],
        checks: vec![check],
        assessment: Some(CompletionAssessment {
            summary: "Implemented and tested".into(),
            coverage: vec![RequirementCoverage {
                requirement_id: "goal".into(),
                evidence_ids: vec!["check-1".into()],
                explanation: "The regression tests pass".into(),
            }],
        }),
        blockers: vec![],
        changed_paths: vec![],
        supervision: None,
    };
    (root, snapshot, report)
}

fn gate(report: &mut TaskReport, snapshot: &WorkspaceSnapshot) {
    assess_with_identity(report, snapshot, "fixture-toolchain").unwrap();
}

#[test]
fn valid_full_test_and_coverage_complete() {
    let (_root, snapshot, mut report) = fixture();
    gate(&mut report, &snapshot);
    assert_eq!(report.status, TaskStatus::Complete);
}

#[test]
fn invalid_supervision_counters_do_not_pass_the_gate() {
    let (_root, snapshot, mut report) = fixture();
    report.supervision = Some(z_engine_runtime::SupervisionReport {
        continuations: 4,
        max_continuations: 3,
        last_action: z_engine_runtime::SupervisionAction::Complete,
        reason: "untrusted recorded claim".into(),
    });
    assert!(assess_with_identity(&mut report, &snapshot, "fixture-toolchain").is_err());
    assert_ne!(report.status, TaskStatus::Complete);
}

#[test]
fn old_all_targets_test_evidence_cannot_complete() {
    let (_root, snapshot, mut report) = fixture();
    report.checks[0].command.insert(3, "--all-targets".into());
    gate(&mut report, &snapshot);
    assert_eq!(report.status, TaskStatus::Stale);
    assert_eq!(report.checks[0].outcome, CheckOutcome::Stale);
}

#[test]
fn missing_assessment_coverage_or_explanation_never_completes() {
    for kind in 0..5 {
        let (_root, snapshot, mut report) = fixture();
        match kind {
            0 => report.assessment = None,
            1 => report.assessment.as_mut().unwrap().coverage.clear(),
            2 => report.assessment.as_mut().unwrap().coverage[0].explanation = " ".into(),
            3 => report.assessment.as_mut().unwrap().summary.clear(),
            _ => report.requirements.push(Requirement {
                id: "extra".into(),
                description: "another requirement".into(),
            }),
        }
        gate(&mut report, &snapshot);
        assert_eq!(report.status, TaskStatus::NeedsVerification);
    }
}

#[test]
fn unknown_evidence_and_requirement_ids_never_complete() {
    for requirement in [false, true] {
        let (_root, snapshot, mut report) = fixture();
        let coverage = &mut report.assessment.as_mut().unwrap().coverage[0];
        if requirement {
            coverage.requirement_id = "../../goal".into();
        } else {
            coverage.evidence_ids = vec!["made-up".into()];
        }
        gate(&mut report, &snapshot);
        assert_ne!(report.status, TaskStatus::Complete);
    }
}

#[test]
fn changed_missing_or_corrupt_artifacts_are_stale() {
    for kind in 0..4 {
        let (_root, snapshot, mut report) = fixture();
        match kind {
            0 => report.checks[0].input_fingerprint = Some("old".into()),
            1 => std::fs::remove_file(&report.checks[0].stdout.as_ref().unwrap().path).unwrap(),
            2 => {
                std::fs::write(&report.checks[0].stderr.as_ref().unwrap().path, "tampered").unwrap()
            }
            _ => report.checks[0].toolchain = "old compiler".into(),
        }
        gate(&mut report, &snapshot);
        assert_eq!(report.status, TaskStatus::Stale);
    }
}

#[test]
fn partial_build_and_zero_tests_never_complete() {
    for kind in 0..4 {
        let (_root, snapshot, mut report) = fixture();
        match kind {
            0 => report.checks[0].spec.package = Some("verification-fixture".into()),
            1 => report.checks[0].spec.filter = Some("value".into()),
            2 => report.checks[0].spec.kind = CheckKind::CargoBuild,
            _ => report.checks[0].tests_run = Some(0),
        }
        report.checks[0].command = report.checks[0].spec.command().unwrap();
        gate(&mut report, &snapshot);
        assert_ne!(report.status, TaskStatus::Complete);
    }
}

#[test]
fn latest_failures_block_but_repaired_older_failure_does_not() {
    for newest in [false, true] {
        let (_root, snapshot, mut report) = fixture();
        let mut failure = report.checks[0].clone();
        failure.id = "failed".into();
        failure.exit_code = Some(1);
        failure.outcome = CheckOutcome::Failed;
        if newest {
            report.checks.push(failure);
        } else {
            report.checks.insert(0, failure);
        }
        gate(&mut report, &snapshot);
        assert_eq!(
            report.status,
            if newest {
                TaskStatus::Blocked
            } else {
                TaskStatus::Complete
            }
        );
    }
}

#[test]
fn parent_blockers_survive_and_schemas_are_validated() {
    let (_root, snapshot, mut report) = fixture();
    report.blockers.push("external edit scope".into());
    gate(&mut report, &snapshot);
    assert_eq!(report.status, TaskStatus::Blocked);
    assert_eq!(report.blockers, ["external edit scope"]);
    report.schema_version = 2;
    assert!(assess_with_identity(&mut report, &snapshot, "fixture-toolchain").is_err());
    assert_ne!(report.status, TaskStatus::Complete);
}

#[test]
fn refresh_never_promotes_and_missing_workspace_invalidates_complete() {
    let (root, _, mut report) = fixture();
    report.status = TaskStatus::Interrupted;
    report.checks.clear();
    refresh(&mut report).unwrap();
    assert_eq!(report.status, TaskStatus::Interrupted);
    std::fs::remove_file(root.0.join("Cargo.toml")).unwrap();
    refresh(&mut report).unwrap();
    assert_eq!(report.status, TaskStatus::Interrupted);
    report.status = TaskStatus::Complete;
    assert!(refresh(&mut report).is_err());
    assert_eq!(report.status, TaskStatus::Stale);
}
