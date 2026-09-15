//! Completion is a durable, current full-workspace result, not a model's closing prose.

#[path = "support/verification_contract.rs"]
mod verification_contract;
#[path = "support/verification_fixture.rs"]
mod verification_fixture;
#[path = "support/verification_provider.rs"]
mod verification_provider;

use std::path::Path;

use serde_json::json;
use sha2::{Digest, Sha256};
use z_engine_core::agent::Event;
use z_engine_core::session::{SessionEvent, replay};
use z_engine_core::verification::{CheckKind, CheckOutcome, TaskStatus};

use verification_fixture::{Fixture, GOAL};
use verification_provider::{Step, check_evidence, edit, read, repair_steps, tool_result, verify};

#[tokio::test]
async fn failed_test_then_managed_fix_requires_current_full_run_and_durable_coverage() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.push(Step::Done);
    let observed = fixture.run(steps, true, false).await;
    observed.assert_response_completed();
    let report = observed.final_report();
    assert_eq!(report.status, TaskStatus::Complete, "{report:#?}");
    assert_eq!(report.goal, GOAL);
    assert_eq!(report.workspace_root, fixture.workspace.to_string_lossy());
    assert_eq!(report.requirements.len(), 1);
    assert_eq!(report.requirements[0].id, "goal");
    assert_eq!(report.requirements[0].description, GOAL);
    assert_eq!(report.changed_paths, ["src/lib.rs"]);
    assert_eq!(report.checks.len(), 2);
    let failed = &report.checks[0];
    let passed = &report.checks[1];
    assert_eq!(failed.outcome, CheckOutcome::Failed);
    assert!(failed.exit_code.is_some_and(|code| code != 0));
    assert_eq!(passed.outcome, CheckOutcome::Passed);
    assert_eq!(passed.exit_code, Some(0));
    assert_eq!(passed.tests_run, Some(1));
    assert_eq!(passed.spec.kind, CheckKind::CargoTest);
    assert!(passed.spec.package.is_none());
    assert!(passed.spec.filter.is_none());
    assert_eq!(
        passed.command,
        ["cargo", "test", "--workspace", "--no-fail-fast"]
    );
    assert_ne!(failed.input_fingerprint, passed.input_fingerprint);
    assert!(passed.input_fingerprint.is_some());
    assert!(!passed.toolchain.is_empty());
    let coverage = &report.assessment.as_ref().unwrap().coverage;
    assert_eq!(coverage.len(), 1);
    assert_eq!(coverage[0].requirement_id, "goal");
    assert_eq!(coverage[0].evidence_ids, std::slice::from_ref(&passed.id));
    assert!(!coverage[0].explanation.is_empty());
    assert!(report.blockers.is_empty(), "{:#?}", report.blockers);

    for check in &report.checks {
        for artifact in [
            check.stdout.as_ref().unwrap(),
            check.stderr.as_ref().unwrap(),
        ] {
            let path = Path::new(&artifact.path);
            assert!(path.starts_with(fixture.session.with_extension("artifacts")));
            assert!(!path.starts_with(&fixture.workspace));
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(artifact.digest, format!("{:x}", Sha256::digest(bytes)));
        }
    }
    assert!(
        std::fs::read_to_string(&failed.stdout.as_ref().unwrap().path)
            .unwrap()
            .contains("test result: FAILED.")
    );
    assert!(
        std::fs::read_to_string(&passed.stdout.as_ref().unwrap().path)
            .unwrap()
            .contains("test result: ok. 1 passed")
    );
    let from_tool = check_evidence(observed.requests.last().unwrap(), "verify-pass");
    assert_eq!(&from_tool, passed);

    let reports = observed.reports();
    assert_eq!(reports.last().unwrap().status, TaskStatus::Complete);
    assert!(
        reports[..reports.len() - 1]
            .iter()
            .all(|r| r.status != TaskStatus::Complete)
    );
    let complete_index = observed.events.iter().position(|event| {
        matches!(event, Event::TaskUpdated { report } if report.status == TaskStatus::Complete)
    }).unwrap();
    let assess_index = observed.events.iter().position(|event| {
        matches!(event, Event::ToolCallFinished { name, ok: true, .. } if name == "assess_completion")
    }).unwrap();
    let response_index = observed
        .events
        .iter()
        .position(|event| matches!(event, Event::TurnCompleted { .. }))
        .unwrap();
    let final_text_index = observed
        .events
        .iter()
        .position(|event| matches!(event, Event::TokenDelta(text) if text == "done"))
        .unwrap();
    assert!(assess_index < complete_index && complete_index < response_index);
    assert!(final_text_index < complete_index);

    let persisted_reports: Vec<_> = observed
        .persisted
        .iter()
        .filter_map(|event| match event {
            SessionEvent::TaskUpdated { report } => Some(report.as_ref()),
            _ => None,
        })
        .collect();
    assert_eq!(persisted_reports, reports);
    for event in &observed.persisted {
        let encoded = serde_json::to_vec(event).unwrap();
        assert_eq!(
            serde_json::from_slice::<SessionEvent>(&encoded).unwrap(),
            *event
        );
    }
    let without_reports: Vec<_> = observed
        .persisted
        .iter()
        .filter(|event| !matches!(event, SessionEvent::TaskUpdated { .. }))
        .cloned()
        .collect();
    assert_eq!(
        serde_json::to_value(replay(&observed.persisted).working).unwrap(),
        serde_json::to_value(replay(&without_reports).working).unwrap()
    );
    verification_contract::assert_event_shape(report);
}

#[tokio::test]
async fn done_without_verification_is_only_a_completed_response() {
    let fixture = Fixture::new();
    let observed = fixture.run(vec![Step::Done], true, false).await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert!(observed.final_report().checks.is_empty());
    assert!(observed.final_report().assessment.is_none());
}

#[tokio::test]
async fn denied_verification_cannot_be_replaced_by_done() {
    let fixture = Fixture::new();
    let observed = fixture
        .run(vec![verify("verify-denied"), Step::Done], true, true)
        .await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert!(observed.events.iter().any(|event| {
        matches!(event, Event::ApprovalRequired { tool, .. } if tool == "run_verification")
    }));
    assert!(!observed.events.iter().any(|event| {
        matches!(event, Event::ToolCallStarted { name, .. } if name == "run_verification")
    }));
    assert!(observed.final_report().checks.is_empty());
    assert!(
        tool_result(observed.requests.last().unwrap(), "verify-denied")
            .contains("declined permission")
    );
    assert!(!fixture.session.with_extension("artifacts").exists());
}

#[tokio::test]
async fn denial_after_passing_evidence_and_assessment_still_blocks_completion() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.extend([verify("verify-denied"), Step::Done]);
    let observed = fixture.run_with_denial(steps, true, Some(2)).await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert!(
        observed
            .reports()
            .iter()
            .any(|report| report.assessment.is_some())
    );
    let report = observed.final_report();
    assert_eq!(report.status, TaskStatus::Blocked);
    assert!(report.assessment.is_none());
    assert!(
        report
            .blockers
            .iter()
            .any(|reason| reason.contains("denied"))
    );
    assert_eq!(report.checks.len(), 2);
}

#[tokio::test]
async fn managed_edit_after_pass_and_assessment_invalidates_completion() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.extend([edit("regress-source", "    42\n", "    43\n"), Step::Done]);
    let observed = fixture.run(steps, true, false).await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert!(
        observed
            .reports()
            .iter()
            .any(|report| report.assessment.is_some())
    );
    let report = observed.final_report();
    assert!(report.assessment.is_none());
    assert_eq!(report.checks[1].outcome, CheckOutcome::Stale);
    assert_eq!(report.changed_paths, ["src/lib.rs"]);
    assert!(
        std::fs::read_to_string(fixture.workspace.join("src/lib.rs"))
            .unwrap()
            .contains("    43\n")
    );
}

#[tokio::test]
async fn provider_failure_after_passing_assessment_is_not_completion() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.push(Step::Fail);
    let observed = fixture.run(steps, true, false).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Interrupted);
    assert_eq!(
        observed.final_report().checks[1].outcome,
        CheckOutcome::Passed
    );
    assert!(observed.events.iter().any(|event| {
        matches!(event, Event::Error(message) if message.contains("injected provider failure"))
    }));
    assert!(
        !observed
            .events
            .iter()
            .any(|event| matches!(event, Event::TurnCompleted { .. }))
    );
}

#[tokio::test]
async fn cancellation_after_passing_assessment_is_not_completion() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.push(Step::WaitForCancellation);
    let observed = fixture.run(steps, true, false).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Stopped);
    assert_eq!(
        observed.final_report().checks[1].outcome,
        CheckOutcome::Passed
    );
    assert!(
        observed
            .events
            .iter()
            .any(|event| matches!(event, Event::TurnAborted))
    );
    assert!(
        !observed
            .events
            .iter()
            .any(|event| matches!(event, Event::TurnCompleted { .. }))
    );
}

#[tokio::test]
async fn verification_without_a_durable_recorder_cannot_complete() {
    let fixture = Fixture::new();
    let observed = fixture
        .run(vec![verify("verify-unrecorded"), Step::Done], false, false)
        .await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert!(observed.final_report().checks.is_empty());
    assert!(!observed.final_report().blockers.is_empty());
    assert!(observed.events.iter().any(|event| {
        matches!(event, Event::ToolCallFinished { name, ok: false, .. } if name == "run_verification")
    }));
    assert!(!fixture.session.with_extension("artifacts").exists());
}

#[tokio::test]
async fn passing_build_or_narrow_test_with_coverage_is_not_full_workspace_completion() {
    for input in [
        json!({"kind": "cargo_build", "package": null, "filter": null}),
        json!({"kind": "cargo_test", "package": "verification-fixture", "filter": null}),
        json!({"kind": "cargo_test", "package": null, "filter": "answers_the_original_goal"}),
    ] {
        let fixture = Fixture::new();
        let steps = vec![
            read(),
            edit("fix-source", "    41\n", "    42\n"),
            Step::Tool {
                id: "verify-pass",
                name: "run_verification",
                input: input.clone(),
            },
            Step::Assess,
            Step::Done,
        ];
        let observed = fixture.run(steps, true, false).await;
        observed.assert_response_completed();
        observed.assert_not_complete();
        let report = observed.final_report();
        assert_eq!(report.checks.len(), 1, "{input}: {report:#?}");
        assert_eq!(
            report.checks[0].outcome,
            CheckOutcome::Passed,
            "{input}: {report:#?}"
        );
        assert!(report.assessment.is_some(), "{input}: {report:#?}");
        assert_eq!(report.checks[0].exit_code, Some(0));
    }
}

#[tokio::test]
async fn passing_unit_test_with_failing_doctest_is_not_passing_evidence() {
    let fixture = Fixture::new();
    let path = fixture.workspace.join("src/lib.rs");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("/// ```\n/// assert!(false);\n/// ```\n{source}"),
    )
    .unwrap();
    let observed = fixture
        .run(
            vec![
                read(),
                edit("fix-source", "    41\n", "    42\n"),
                verify("verify-doc"),
                Step::Done,
            ],
            true,
            false,
        )
        .await;
    observed.assert_not_complete();
    assert_eq!(
        observed.final_report().checks[0].outcome,
        CheckOutcome::Failed
    );
}

#[tokio::test]
async fn stop_during_final_assessment_cannot_publish_complete() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.push(Step::Done);
    let observed = fixture.run_aborting_finalization(steps).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Stopped);
    assert!(
        observed
            .events
            .iter()
            .any(|event| matches!(event, Event::TurnAborted))
    );
}

#[tokio::test]
async fn command_launching_env_does_not_preserve_passing_evidence() {
    let fixture = Fixture::new();
    let mut steps = repair_steps();
    steps.extend([
        Step::Tool {
            id: "external-effect",
            name: "bash",
            input: json!({"command":"env touch ../external.txt"}),
        },
        Step::Done,
    ]);
    let observed = fixture.run(steps, true, false).await;
    observed.assert_not_complete();
    assert!(
        fixture
            .workspace
            .parent()
            .unwrap()
            .join("external.txt")
            .exists()
    );
    assert_eq!(observed.final_report().status, TaskStatus::Blocked);
}
