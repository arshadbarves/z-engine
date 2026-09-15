use serde_json::json;
use z_engine_core::agent::Event;
use z_engine_core::verification::TaskReport;

pub fn assert_event_shape(report: &TaskReport) {
    let passed = &report.checks[1];
    let event = serde_json::to_value(Event::TaskUpdated {
        report: report.clone(),
    })
    .unwrap();
    assert_eq!(event["type"], "taskUpdated");
    assert_eq!(event["report"]["schemaVersion"], 1);
    assert_eq!(event["report"]["taskId"], report.task_id);
    assert_eq!(event["report"]["status"], "complete");
    assert_eq!(event["report"]["changedPaths"], json!(["src/lib.rs"]));
    assert_eq!(event["report"]["checks"][1]["testsRun"], 1);
    assert_eq!(
        event["report"]["assessment"]["coverage"][0]["requirementId"],
        "goal"
    );
    assert_eq!(
        event["report"]["assessment"]["coverage"][0]["evidenceIds"],
        json!([passed.id])
    );
    assert!(event["report"].get("changed_paths").is_none());
}
