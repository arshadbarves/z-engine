//! Capability suggestions travel through normal tools, policy and persistence.

#[allow(dead_code)]
#[path = "support/verification_fixture.rs"]
mod verification_fixture;
#[allow(dead_code)]
#[path = "support/verification_provider.rs"]
mod verification_provider;

use serde_json::json;
use z_engine_core::agent::Event;
use z_engine_core::session::SessionEvent;
use z_engine_core::verification::TaskStatus;
use z_engine_project::{ExecutionStatus, ProjectKind, ProjectReport};

use verification_fixture::Fixture;
use verification_provider::{Step, edit, read, tool_result, verify};

fn inspect(id: &'static str) -> Step {
    Step::Tool {
        id,
        name: "inspect_project",
        input: json!({}),
    }
}

#[tokio::test]
async fn discovery_is_read_only_persisted_and_never_substitutes_for_verification() {
    let fixture = Fixture::new();
    let observed = fixture
        .run(
            vec![
                inspect("inspect-before-edit"),
                read(),
                edit("fix-source", "    41\n", "    42\n"),
                verify("verify-pass"),
                inspect("inspect-after-verification"),
                Step::Assess,
                Step::Done,
            ],
            true,
            false,
        )
        .await;
    observed.assert_response_completed();
    for (index, id) in [
        (1, "inspect-before-edit"),
        (5, "inspect-after-verification"),
    ] {
        let report: ProjectReport =
            serde_json::from_str(tool_result(&observed.requests[index], id)).unwrap();
        assert_eq!(report.execution, ExecutionStatus::NotRun);
        assert!(!report.has_errors());
        assert!(report.profiles.iter().any(|profile| {
            profile.kind == ProjectKind::Cargo && profile.root == fixture.workspace
        }));
        assert!(observed.persisted.iter().any(|event| {
            matches!(event, SessionEvent::ToolResult { tool_call_id, .. } if tool_call_id == id)
        }));
    }
    assert!(!observed.events.iter().any(|event| {
        matches!(event, Event::ApprovalRequired { tool, .. } if tool == "inspect_project")
    }));
    let final_report = observed.final_report();
    assert_eq!(
        final_report.status,
        TaskStatus::Complete,
        "{final_report:#?}"
    );
    assert_eq!(final_report.checks.len(), 1);
    assert!(final_report.blockers.is_empty());
}
