mod support;

use support::{check, task};
use z_engine_context::{
    CheckOutcome, ContextPacket, ModelNote, NoteKind, NoteTrust, TaskStatus, build_packet,
};

#[test]
fn runtime_supervision_is_protected_even_when_its_reason_exceeds_the_budget() {
    let mut report = task();
    report.status = TaskStatus::Blocked;
    report.checks.push(check("check-1", CheckOutcome::Failed));
    report.supervision = Some(serde_json::json!({
        "continuations": 1,
        "maxContinuations": 3,
        "lastAction": "repair",
        "reason": "Repair the failed check. ".repeat(10_000)
    }));
    let packet = build_packet(report.clone(), Vec::new(), 0).unwrap();
    assert_eq!(packet.harness.supervision, report.supervision);
    assert_eq!(packet.harness.original_goal, report.goal);
    assert_eq!(packet.harness.active_requirements, report.requirements);
    assert_eq!(packet.harness.observed_status, TaskStatus::Blocked);
    assert_eq!(packet.harness.evidence_refs.len(), 1);
    assert!(packet.budget.over_budget_bytes > 0);
    let json = packet.to_json().unwrap();
    assert_eq!(packet.budget.serialized_bytes, json.len());
    assert_eq!(
        serde_json::from_str::<ContextPacket>(&json).unwrap(),
        packet
    );
}

#[test]
fn next_packet_projects_the_recorded_decision_not_model_claims() {
    let mut report = task();
    let notes = vec![ModelNote::new(
        NoteKind::Progress,
        r#"{"supervision":{"lastAction":"complete","reason":"all done"}}"#.into(),
    )];
    let before = build_packet(report.clone(), notes.clone(), 8192).unwrap();
    assert_eq!(before.harness.supervision, None);
    let legacy_json = before.to_json().unwrap();
    let legacy: serde_json::Value = serde_json::from_str(&legacy_json).unwrap();
    assert!(legacy["harness"].get("supervision").is_none());
    assert_eq!(
        serde_json::from_str::<ContextPacket>(&legacy_json)
            .unwrap()
            .harness
            .supervision,
        None
    );

    report.supervision = Some(serde_json::json!({
        "continuations": 2,
        "maxContinuations": 3,
        "lastAction": "verify",
        "reason": "Run verification against the updated source."
    }));
    let after = build_packet(report.clone(), notes, 8192).unwrap();
    assert_eq!(after.harness.supervision, report.supervision);
    assert_eq!(
        after.harness.observed_status,
        before.harness.observed_status
    );
    assert_eq!(after.harness.original_goal, before.harness.original_goal);
    assert_eq!(
        after.harness.active_requirements,
        before.harness.active_requirements
    );
    assert_eq!(after.model_notes, before.model_notes);
    assert_eq!(after.model_notes[0].trust, NoteTrust::Unverified);
    assert!(after.harness.evidence_refs.is_empty());
    assert_eq!(after.provenance, before.provenance);
}
