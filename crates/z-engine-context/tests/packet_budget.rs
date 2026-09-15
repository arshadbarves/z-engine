mod support;

use support::{check, task};
use z_engine_context::{CheckOutcome, ModelNote, NoteKind, build_packet};

#[test]
fn protected_content_is_never_truncated_even_with_zero_budget() {
    let mut report = task();
    report.goal = format!(
        "start {} original-goal-end",
        "\u{1f980}\"\\\n".repeat(10_000)
    );
    report.requirements[0].description = format!("{} requirement-end", "required ".repeat(10_000));
    report.blockers = vec!["blocked ".repeat(10_000)];
    report.checks.push(check("check-1", CheckOutcome::Failed));
    report.changed_paths.push("src/parser.rs".into());
    let notes = vec![ModelNote::new(
        NoteKind::Progress,
        "everything is complete".into(),
    )];

    let packet = build_packet(report.clone(), notes, 0).unwrap();
    assert_eq!(packet.harness.original_goal, report.goal);
    assert_eq!(packet.harness.active_requirements, report.requirements);
    assert_eq!(packet.harness.blockers, report.blockers);
    assert_eq!(packet.harness.evidence_refs.len(), 1);
    assert!(packet.evidence_details.is_empty());
    assert!(packet.changed_paths.is_empty());
    assert!(packet.model_notes.is_empty());
    assert_eq!(packet.omitted.evidence_details, 1);
    assert_eq!(packet.omitted.changed_paths, 1);
    assert_eq!(packet.omitted.model_notes, 1);
    assert_eq!(
        packet.budget.serialized_bytes,
        packet.to_json().unwrap().len()
    );
    assert_eq!(
        packet.budget.over_budget_bytes,
        packet.budget.serialized_bytes
    );
}

#[test]
fn replacement_summaries_survive_even_when_protected_task_data_overflows() {
    let summary = ModelNote::new(
        NoteKind::Summary,
        "Prior investigation found the cause.".into(),
    );
    let mut report = task();
    report.goal = "original goal ".repeat(1024);
    let notes = vec![
        ModelNote::new(NoteKind::Progress, "optional progress".into()),
        summary.clone(),
    ];
    for budget in [0, 1024, 32_768] {
        let packet = build_packet(report.clone(), notes.clone(), budget).unwrap();
        assert_eq!(packet.model_notes[0], summary);
        assert_eq!(
            packet.model_notes[0].trust,
            z_engine_context::NoteTrust::Unverified
        );
        assert_eq!(
            packet.budget.serialized_bytes,
            packet.to_json().unwrap().len()
        );
        assert_eq!(
            packet.omitted.model_notes,
            usize::from(packet.budget.over_budget_bytes > 0)
        );
    }
}

#[test]
fn long_request_and_duplicated_goal_requirement_have_no_fixed_protected_cap() {
    let mut report = task();
    report.goal = format!("{}accepted-goal-end", "accepted requirement ".repeat(1024));
    report.requirements[0].description = report.goal.clone();
    assert!(report.goal.len() > 15 * 1024);
    let notes = vec![ModelNote::new(
        NoteKind::Progress,
        "unverified progress".into(),
    )];
    let small = build_packet(report.clone(), notes.clone(), 8 * 1024).unwrap();
    assert!(small.budget.over_budget_bytes > 0);
    assert_eq!(small.harness.original_goal, report.goal);
    assert_eq!(small.harness.active_requirements, report.requirements);
    assert!(small.model_notes.is_empty());
    let large = build_packet(report.clone(), notes, 128 * 1024).unwrap();
    assert_eq!(large.budget.over_budget_bytes, 0);
    assert_eq!(large.harness.original_goal, report.goal);
    assert_eq!(large.harness.active_requirements, report.requirements);
    assert_eq!(
        large.model_notes[0].trust,
        z_engine_context::NoteTrust::Unverified
    );
    let json = large.to_json().unwrap();
    assert_eq!(json.len(), large.budget.serialized_bytes);
    let envelope: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(envelope["kind"], "task_context");
    assert_eq!(envelope["harness"]["originalGoal"], report.goal);
}

#[test]
fn oversized_optional_entry_is_skipped_without_starving_later_entries() {
    let small = ModelNote::new(NoteKind::NeedsLater, "inspect parser".into());
    let capacity = build_packet(task(), vec![small.clone()], 4096)
        .unwrap()
        .budget
        .serialized_bytes
        + 16;
    let notes = vec![
        ModelNote::new(NoteKind::Progress, "optional ".repeat(10_000)),
        small.clone(),
    ];
    let first = build_packet(task(), notes.clone(), capacity).unwrap();
    let second = build_packet(task(), notes, capacity).unwrap();
    assert_eq!(first.model_notes, [small]);
    assert_eq!(first.omitted.model_notes, 1);
    assert_eq!(first.budget.over_budget_bytes, 0);
    assert!(first.budget.serialized_bytes <= capacity);
    assert_eq!(first.to_json().unwrap(), second.to_json().unwrap());
}

#[test]
fn compact_json_bytes_include_escaping_metadata_and_the_byte_count_itself() {
    let notes = vec![ModelNote::new(
        NoteKind::Progress,
        "\"\n\\\u{1f980}".repeat(20),
    )];
    for target in [0, 1, 128, 1000, 2048, 8192, usize::MAX] {
        let packet = build_packet(task(), notes.clone(), target).unwrap();
        let bytes = packet.to_json().unwrap().len();
        assert_eq!(packet.budget.serialized_bytes, bytes);
        assert_eq!(
            packet.budget.over_budget_bytes,
            bytes.saturating_sub(target)
        );
        if bytes > target {
            assert!(packet.model_notes.is_empty());
            assert_eq!(packet.omitted.model_notes, 1);
        }
    }
}

#[test]
fn optional_entry_fits_at_exact_budget_but_not_one_byte_below() {
    let note = ModelNote::new(NoteKind::Decision, "keep API".into());
    let first = build_packet(task(), vec![note.clone()], 4096).unwrap();
    let second = build_packet(task(), vec![note.clone()], first.budget.serialized_bytes).unwrap();
    let exact = second.budget.serialized_bytes;
    let at_limit = build_packet(task(), vec![note.clone()], exact).unwrap();
    assert_eq!(at_limit.model_notes.as_slice(), std::slice::from_ref(&note));
    assert_eq!(at_limit.budget.serialized_bytes, exact);
    assert_eq!(at_limit.omitted.model_notes, 0);
    let below_limit = build_packet(task(), vec![note], exact - 1).unwrap();
    assert!(below_limit.model_notes.is_empty());
    assert_eq!(below_limit.omitted.model_notes, 1);
    assert_eq!(below_limit.budget.over_budget_bytes, 0);
}

#[test]
fn rejected_entries_do_not_inflate_accounting_at_decimal_boundaries() {
    let mut report = task();
    let notes = vec![ModelNote::new(
        NoteKind::Progress,
        "oversized".repeat(10_000),
    )];
    let initial = build_packet(report.clone(), notes.clone(), 999).unwrap();
    assert!(initial.budget.serialized_bytes < 999);
    report
        .goal
        .push_str(&"x".repeat(999 - initial.budget.serialized_bytes));
    let packet = build_packet(report, notes, 999).unwrap();
    assert_eq!(packet.budget.serialized_bytes, 999);
    assert_eq!(packet.to_json().unwrap().len(), 999);
    assert_eq!(packet.budget.over_budget_bytes, 0);
    assert!(packet.model_notes.is_empty());
    assert_eq!(packet.omitted.model_notes, 1);
}

#[test]
fn large_budget_includes_all_optional_context_without_overflow() {
    let mut report = task();
    report.checks.push(check("check-1", CheckOutcome::Passed));
    report.changed_paths = vec!["src/parser.rs".into(), "src/lib.rs".into()];
    let notes = vec![ModelNote::new(NoteKind::Summary, "old summary".into())];
    let packet = build_packet(report, notes, usize::MAX).unwrap();
    assert_eq!(packet.evidence_details.len(), 1);
    assert_eq!(packet.changed_paths, ["src/parser.rs", "src/lib.rs"]);
    assert_eq!(packet.model_notes.len(), 1);
    assert_eq!(packet.omitted, Default::default());
    assert_eq!(packet.budget.over_budget_bytes, 0);
}

#[test]
fn nonpassing_newest_evidence_has_priority_and_omissions_are_exact() {
    let mut report = task();
    report.checks = vec![
        check("check-1", CheckOutcome::Failed),
        check("check-2", CheckOutcome::Passed),
        check("check-3", CheckOutcome::Stale),
    ];
    let all = build_packet(report.clone(), Vec::new(), 8192).unwrap();
    assert_eq!(
        all.evidence_details
            .iter()
            .map(|d| d.evidence_id.as_str())
            .collect::<Vec<_>>(),
        ["check-3", "check-1", "check-2"]
    );
    let minimum = build_packet(report.clone(), Vec::new(), 0)
        .unwrap()
        .budget
        .serialized_bytes;
    let first_detail_bytes = serde_json::to_vec(&all.evidence_details[0]).unwrap().len();
    let target = minimum + first_detail_bytes + 20;
    let packet = build_packet(report.clone(), Vec::new(), target).unwrap();
    assert_eq!(packet.evidence_details.len(), 1);
    assert_eq!(packet.evidence_details[0].evidence_id, "check-3");
    assert_eq!(packet.harness.evidence_refs.len(), 3);
    assert_eq!(packet.omitted.evidence_details, 2);
    assert!(packet.budget.serialized_bytes <= target);
    assert_eq!(
        packet.to_json().unwrap(),
        build_packet(report, Vec::new(), target)
            .unwrap()
            .to_json()
            .unwrap()
    );
}
