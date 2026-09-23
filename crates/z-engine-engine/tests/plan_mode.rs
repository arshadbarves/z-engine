//! Plan mode: writes are refused, `ExitPlanMode` opens a plan review, and
//! approving it with acceptEdits switches the mode so the edit then runs.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text, results};
use z_engine_protocol::{Command, Event, PermissionMode, PlanDecision};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn approved_plan_switches_mode_and_unblocks_writes() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"plan\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let write = json!({ "file_path": "plan.txt", "content": "implemented" });
    h.model.push(Script::tool("Write", write.clone()));
    h.model.push(Script::tool(
        "ExitPlanMode",
        json!({ "plan": "1. Write plan.txt" }),
    ));
    h.model.push(Script::tool("Write", write));
    h.model.push(Script::text("Done."));
    h.submit("plan and then write the file");

    let request_id = match h.wait(|e| matches!(e, Event::PlanProposed { .. })).await {
        Event::PlanProposed {
            request_id, plan, ..
        } => {
            assert_eq!(plan, "1. Write plan.txt");
            request_id
        }
        other => panic!("{other:?}"),
    };
    h.send(Command::ResolvePlan {
        request_id,
        decision: PlanDecision::Approve {
            mode: PermissionMode::AcceptEdits,
            edited_plan: None,
        },
    });
    h.wait(|e| {
        matches!(
            e,
            Event::ModeChanged {
                mode: PermissionMode::AcceptEdits
            }
        )
    })
    .await;
    h.turn_finished().await;

    let requests = h.main_requests();
    assert!(last_user_text(&requests[0]).contains("Plan mode is active"));
    let refused = results(requests[1].messages.last().unwrap());
    assert!(
        refused[0].1 && refused[0].2.contains("plan mode"),
        "{:?}",
        refused[0]
    );
    let approved = results(requests[2].messages.last().unwrap());
    assert!(approved[0].2.contains("approved"), "{:?}", approved[0]);
    assert_eq!(h.repo.read("plan.txt"), "implemented");
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::PlanResolved { approved: true, .. })),
        1
    );

    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert_eq!(snapshot.info.mode, PermissionMode::AcceptEdits);
            assert!(snapshot.pending_plans.is_empty());
        }
        other => panic!("{other:?}"),
    }
}
