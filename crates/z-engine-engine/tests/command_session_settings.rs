//! `/model`, `/mode` and `/effort` change the session like the matching
//! GUI controls and confirm with command output.

mod support;

use support::Harness;
use z_engine_protocol::{Effort, Event, PermissionMode};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn model_mode_and_effort_commands() {
    let mut h = Harness::start(FixtureRepo::empty()).await;

    h.command("model", "other-model");
    h.expect(|e| matches!(e, Event::ModelChanged { model } if model == "other-model"))
        .await;
    assert!(h.command_output("model").await.contains("`other-model`"));

    h.command("mode", "plan");
    h.expect(|e| {
        matches!(
            e,
            Event::ModeChanged {
                mode: PermissionMode::Plan
            }
        )
    })
    .await;
    assert!(h.command_output("mode").await.contains("plan"));

    h.command("effort", "high");
    h.expect(|e| {
        matches!(
            e,
            Event::EffortChanged {
                effort: Some(Effort::High)
            }
        )
    })
    .await;
    assert!(h.command_output("effort").await.contains("high"));
    h.command("effort", "default");
    h.expect(|e| matches!(e, Event::EffortChanged { effort: None }))
        .await;

    h.command("mode", "sideways");
    h.notice("Usage: /mode").await;

    h.model.push(Script::text("ok"));
    h.run_turn("hi").await;
    assert_eq!(h.main_requests().pop().unwrap().model, "other-model");
}
