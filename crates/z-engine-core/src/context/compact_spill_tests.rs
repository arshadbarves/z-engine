use super::*;

#[test]
fn failed_spill_keeps_original_context_and_returns_the_io_cause() {
    let tmp = tempfile::tempdir().unwrap();
    let blocked = tmp.path().join("not-a-directory");
    std::fs::write(&blocked, "occupied").unwrap();
    let mut msgs = vec![ChatMessage::tool_result(
        "c1",
        "[harness:tool-output id=x]\noriginal",
    )];
    let before = serde_json::to_value(&msgs).unwrap();
    let ids = BTreeSet::from(["x".to_string()]);
    assert!(matches!(
        elide_droppable(&mut msgs, &ids, &blocked),
        Err(CompactionError::Io { .. })
    ));
    assert_eq!(serde_json::to_value(&msgs).unwrap(), before);
    assert!(matches!(
        compact(&msgs, 0, &blocked),
        Err(CompactionError::Io { .. })
    ));
    assert_eq!(serde_json::to_value(&msgs).unwrap(), before);
}

#[test]
fn later_spill_failure_does_not_partially_elide_the_transcript() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing.log");
    let mut msgs = vec![
        ChatMessage::tool_result("c1", "[harness:tool-output id=x]\noriginal"),
        ChatMessage::tool_result(
            "c2",
            format!(
                "[harness:tool-output id=y]\n[harness:elided; full: {}]",
                missing.display()
            ),
        ),
    ];
    let before = serde_json::to_value(&msgs).unwrap();
    let ids = BTreeSet::from(["x".to_string(), "y".to_string()]);
    assert!(elide_droppable(&mut msgs, &ids, tmp.path()).is_err());
    assert_eq!(serde_json::to_value(&msgs).unwrap(), before);
    let stored: Vec<_> = std::fs::read_dir(tmp.path().join("z-engine"))
        .unwrap()
        .collect();
    assert_eq!(stored.len(), 1);
    assert_eq!(
        std::fs::read_to_string(stored[0].as_ref().unwrap().path()).unwrap(),
        "[harness:tool-output id=x]\noriginal"
    );
}

#[test]
fn embedded_marker_cannot_replace_actual_output_with_an_unrelated_file() {
    let tmp = tempfile::tempdir().unwrap();
    let unrelated = tmp.path().join("unrelated.log");
    std::fs::write(&unrelated, "unrelated").unwrap();
    let original = format!(
        "important prefix\n[harness:elided; full: {}]\nimportant suffix",
        unrelated.display()
    );
    let elided = elide_text(&original, tmp.path()).unwrap().unwrap();
    let stored = existing_spill_path(&elided).unwrap();
    assert_ne!(stored, unrelated);
    assert_eq!(std::fs::read_to_string(stored).unwrap(), original);
}

#[test]
fn elide_droppable_preserves_ids_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let original = "[harness:tool-output id=abcd1234]\nlong thing\n";
    let mut msgs = vec![
        ChatMessage::tool_result("k", original),
        ChatMessage::user("later"),
        ChatMessage::tool_result("m", "fresh output"),
    ];
    let ids = BTreeSet::from(["abcd1234".to_string()]);
    assert_eq!(elide_droppable(&mut msgs, &ids, tmp.path()).unwrap(), 1);
    let ChatMessage::Tool { content, .. } = &msgs[0] else {
        panic!("tool result")
    };
    assert!(content.contains("id=abcd1234"));
    let stored = existing_spill_path(content).unwrap();
    assert_eq!(std::fs::read_to_string(stored).unwrap(), original);
    let before = serde_json::to_value(&msgs).unwrap();
    assert_eq!(elide_droppable(&mut msgs, &ids, tmp.path()).unwrap(), 0);
    assert_eq!(serde_json::to_value(&msgs).unwrap(), before);
}
