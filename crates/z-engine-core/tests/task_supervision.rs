//! The same durable task can continue beyond a premature closing response.

#[allow(dead_code)]
#[path = "support/verification_fixture.rs"]
mod verification_fixture;
#[allow(dead_code)]
#[path = "support/verification_provider.rs"]
mod verification_provider;

use z_engine_core::agent::Event;
use z_engine_core::session::SessionEvent;
use z_engine_core::verification::TaskStatus;
use z_engine_runtime::SupervisionAction;

use verification_fixture::Fixture;
use verification_provider::{Step, edit, read, verify};

fn edit_then_stop() -> Vec<Step> {
    vec![
        read(),
        edit("fix-source", "    41\n", "    42\n"),
        Step::Done,
    ]
}

#[tokio::test]
async fn premature_answer_after_edit_continues_same_task_to_durable_completion() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.extend([verify("verify-pass"), Step::Assess, Step::Done]);
    let observed = fixture.run_supervised(steps, 3, None).await;
    observed.assert_response_completed();
    let report = observed.final_report();
    assert_eq!(report.status, TaskStatus::Complete, "{report:#?}");
    let supervision = report.supervision.as_ref().unwrap();
    assert_eq!(supervision.continuations, 1);
    assert_eq!(supervision.last_action, SupervisionAction::Complete);
    assert!(
        observed
            .reports()
            .iter()
            .all(|item| item.task_id == report.task_id)
    );
    assert_eq!(
        observed
            .persisted
            .iter()
            .filter(|event| matches!(event, SessionEvent::UserMsg { .. }))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|event| matches!(event, Event::TurnCompleted { .. }))
            .count(),
        1
    );
    assert!(observed.persisted.iter().any(|event| {
        matches!(event, SessionEvent::TaskUpdated { report }
            if report.supervision.as_ref().is_some_and(|s| s.last_action == SupervisionAction::Verify))
    }));
    let continuation_request = &observed.requests[3];
    let messages = continuation_request["messages"].as_array().unwrap();
    let newest = messages.last().unwrap();
    assert_eq!(newest["role"], "user");
    let packet: serde_json::Value =
        serde_json::from_str(newest["content"].as_str().unwrap()).unwrap();
    assert_eq!(packet["kind"], "task_context");
    assert_eq!(
        packet["harness"]["originalGoal"],
        verification_fixture::GOAL
    );
    assert_eq!(packet["harness"]["supervision"]["lastAction"], "verify");
    assert_eq!(packet["harness"]["supervision"]["continuations"], 1);
    assert!(messages[..messages.len() - 1].iter().all(|message| {
        message["content"]
            .as_str()
            .and_then(|text| serde_json::from_str::<z_engine_context::ContextPacket>(text).ok())
            .is_none()
    }));
}

#[tokio::test]
async fn repeated_done_without_progress_blocks_instead_of_looping() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.push(Step::Done);
    let observed = fixture.run_supervised(steps, 3, None).await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    let report = observed.final_report();
    assert_eq!(report.status, TaskStatus::Blocked);
    assert_eq!(report.supervision.as_ref().unwrap().continuations, 1);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("recurred"))
    );
}

#[tokio::test]
async fn a_failed_check_can_trigger_investigation_and_repair() {
    let fixture = Fixture::new();
    let observed = fixture
        .run_supervised(
            vec![
                read(),
                verify("verify-fail"),
                Step::Done,
                edit("fix-source", "    41\n", "    42\n"),
                verify("verify-pass"),
                Step::Assess,
                Step::Done,
            ],
            3,
            None,
        )
        .await;
    observed.assert_response_completed();
    assert_eq!(observed.final_report().status, TaskStatus::Complete);
    assert!(observed.reports().iter().any(|report| {
        report
            .supervision
            .as_ref()
            .is_some_and(|state| state.last_action == SupervisionAction::Repair)
    }));
}

#[tokio::test]
async fn new_check_ids_do_not_make_repeated_identical_checks_progress() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.extend([
        verify("verify-pass"),
        Step::Done,
        verify("verify-repeat"),
        Step::Done,
    ]);
    let observed = fixture.run_supervised(steps, 3, None).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Blocked);
    assert_eq!(
        observed
            .final_report()
            .supervision
            .as_ref()
            .unwrap()
            .continuations,
        2
    );
}

#[tokio::test]
async fn changing_inputs_cannot_bypass_the_continuation_budget() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.extend([edit("another-change", "    42\n", "    43\n"), Step::Done]);
    let observed = fixture.run_supervised(steps, 1, None).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Blocked);
    assert!(
        observed
            .final_report()
            .blockers
            .iter()
            .any(|blocker| blocker.contains("budget"))
    );
}

#[tokio::test]
async fn denied_check_during_continuation_does_not_trigger_another_attempt() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.extend([verify("verify-denied"), Step::Done]);
    let observed = fixture.run_supervised(steps, 3, Some(0)).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Blocked);
    assert_eq!(
        observed
            .final_report()
            .supervision
            .as_ref()
            .unwrap()
            .continuations,
        1
    );
}

#[tokio::test]
async fn stop_during_continuation_is_cancelled_not_restarted() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.push(Step::WaitForCancellation);
    let observed = fixture.run_supervised(steps, 3, None).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Stopped);
    assert_eq!(
        observed
            .final_report()
            .supervision
            .as_ref()
            .unwrap()
            .last_action,
        SupervisionAction::Stopped
    );
    assert!(
        observed
            .events
            .iter()
            .any(|event| matches!(event, Event::TurnAborted))
    );
}

#[tokio::test]
async fn provider_failure_during_continuation_remains_interrupted() {
    let fixture = Fixture::new();
    let mut steps = edit_then_stop();
    steps.push(Step::Fail);
    let observed = fixture.run_supervised(steps, 3, None).await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Interrupted);
}

#[tokio::test]
async fn read_only_answer_does_not_trigger_automatic_extra_work() {
    let fixture = Fixture::new();
    let observed = fixture
        .run_supervised(vec![read(), Step::Done], 3, None)
        .await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert_eq!(observed.requests.len(), 2);
    assert_eq!(
        observed
            .final_report()
            .supervision
            .as_ref()
            .unwrap()
            .last_action,
        SupervisionAction::Idle
    );
}

#[tokio::test]
async fn malformed_tool_receives_a_bounded_correction_opportunity() {
    let fixture = Fixture::new();
    let observed = fixture
        .run_supervised(vec![Step::MalformedTool, read(), Step::Done], 3, None)
        .await;
    observed.assert_response_completed();
    observed.assert_not_complete();
    assert!(
        verification_provider::tool_result(&observed.requests[1], "malformed")
            .contains("not valid JSON")
    );
}

#[tokio::test]
async fn repeated_malformed_tools_end_as_interrupted_not_complete() {
    let fixture = Fixture::new();
    let observed = fixture
        .run_supervised(
            vec![
                Step::MalformedTool,
                Step::MalformedTool,
                Step::MalformedTool,
            ],
            3,
            None,
        )
        .await;
    observed.assert_not_complete();
    assert_eq!(observed.final_report().status, TaskStatus::Interrupted);
    assert!(observed.events.iter().any(|event| {
        matches!(event, Event::Error(message) if message.contains("correction attempts"))
    }));
}
