use super::*;

fn state() -> LoopState {
    LoopState {
        working: vec![
            ChatMessage::user("original task"),
            ChatMessage::assistant_text("earlier work"),
            ChatMessage::user("recent task"),
        ],
        approval_counter: 0,
        last_usage: Default::default(),
        force_compact: false,
        repo_map_text: None,
        current_task: "original task".into(),
        reasoning_effort: None,
        last_prompt: Arc::new(Mutex::new(None)),
    }
}

#[tokio::test]
async fn stop_cancels_summary_without_replacing_or_persisting_prose() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = state();
    let before = serde_json::to_value(&state.working).unwrap();
    let notes = Arc::new(Mutex::new(NotesStore::default()));
    let mut cfg = LoopConfig::new("fixture", "http://127.0.0.1:1");
    cfg.keep_recent_messages = 1;
    cfg.tmp_dir = tmp.path().into();
    let client = Client::new(&cfg.base_url, None).unwrap();
    let writer = SessionWriter::create(tmp.path()).unwrap();
    let path = writer.path.clone();
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let result = compact_working_set(
        &client,
        &cfg,
        &mut state,
        &notes,
        &events,
        &mut Some(writer),
        &Arc::new(AtomicBool::new(true)),
    )
    .await;
    assert!(matches!(
        result,
        Err(CompactionError::Summary(AuxiliaryError::Cancelled))
    ));
    assert_eq!(serde_json::to_value(&state.working).unwrap(), before);
    assert!(notes.lock().unwrap().is_empty());
    assert!(std::fs::read(path).unwrap().is_empty());
}

#[test]
fn missing_empty_or_whitespace_summary_never_discards_original_context() {
    let tmp = tempfile::tempdir().unwrap();
    for summary in [None, Some(String::new()), Some(" \n\t ".into())] {
        let mut state = state();
        let before = serde_json::to_value(&state.working).unwrap();
        let notes = Mutex::new(NotesStore::default());
        let writer = SessionWriter::create(tmp.path()).unwrap();
        let path = writer.path.clone();
        let outcome = compact::compact(&state.working, 1, tmp.path()).unwrap();
        assert!(matches!(
            commit_compaction(&mut state, &notes, &mut Some(writer), outcome, summary),
            Err(CompactionError::EmptySummary)
        ));
        assert_eq!(serde_json::to_value(&state.working).unwrap(), before);
        assert!(notes.lock().unwrap().is_empty());
        assert!(std::fs::read(path).unwrap().is_empty());
    }
}

#[test]
fn missing_recorder_retains_prose_and_does_not_publish_summary_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = state();
    let before = serde_json::to_value(&state.working).unwrap();
    let notes = Mutex::new(NotesStore::default());
    let outcome = compact::compact(&state.working, 1, tmp.path()).unwrap();
    assert!(matches!(
        commit_compaction(
            &mut state,
            &notes,
            &mut None,
            outcome,
            Some("summary".into())
        ),
        Err(CompactionError::MissingRecorder)
    ));
    assert_eq!(serde_json::to_value(&state.working).unwrap(), before);
    assert!(notes.lock().unwrap().is_empty());
}

#[test]
fn failed_summary_storage_retains_both_original_context_and_existing_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = state();
    let before = serde_json::to_value(&state.working).unwrap();
    let mut existing = NotesStore::default();
    existing.merge(&["existing model note".into()], &[], &[]);
    let notes_before = existing.get().clone();
    let notes = Mutex::new(existing);
    let writer = SessionWriter::create(tmp.path()).unwrap();
    let writer = SessionWriter::from_file_for_test(
        std::fs::File::open(&writer.path).unwrap(),
        writer.path.clone(),
    );
    let outcome = compact::compact(&state.working, 1, tmp.path()).unwrap();
    assert!(matches!(
        commit_compaction(
            &mut state,
            &notes,
            &mut Some(writer),
            outcome,
            Some("summary".into())
        ),
        Err(CompactionError::Storage(_))
    ));
    assert_eq!(serde_json::to_value(&state.working).unwrap(), before);
    assert_eq!(notes.lock().unwrap().get(), &notes_before);
}

#[test]
fn poisoned_notes_lock_prevents_persistence_and_context_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = state();
    let before = serde_json::to_value(&state.working).unwrap();
    let notes = Arc::new(Mutex::new(NotesStore::default()));
    let other = Arc::clone(&notes);
    assert!(
        std::thread::spawn(move || {
            let _guard = other.lock().unwrap();
            panic!("poison notes");
        })
        .join()
        .is_err()
    );
    let writer = SessionWriter::create(tmp.path()).unwrap();
    let path = writer.path.clone();
    let outcome = compact::compact(&state.working, 1, tmp.path()).unwrap();
    assert!(matches!(
        commit_compaction(
            &mut state,
            &notes,
            &mut Some(writer),
            outcome,
            Some("summary".into())
        ),
        Err(CompactionError::NotesLock)
    ));
    assert_eq!(serde_json::to_value(&state.working).unwrap(), before);
    assert!(std::fs::read(path).unwrap().is_empty());
    assert!(matches!(
        elide_marked_outputs(&mut state.working, &notes, tmp.path()),
        Err(CompactionError::NotesLock)
    ));
}

#[test]
fn successful_summary_is_durable_and_unverified_before_prose_is_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = state();
    let notes = Mutex::new(NotesStore::default());
    let writer = SessionWriter::create(tmp.path()).unwrap();
    let path = writer.path.clone();
    let outcome = compact::compact(&state.working, 1, tmp.path()).unwrap();
    let summary = "Model claims the earlier work is complete";
    commit_compaction(
        &mut state,
        &notes,
        &mut Some(writer),
        outcome,
        Some(summary.into()),
    )
    .unwrap();
    assert_eq!(state.working.len(), 1);
    assert!(matches!(&state.working[0], ChatMessage::User { content } if content == "recent task"));
    assert_eq!(state.current_task, "original task");
    let events = crate::session::read_events(&path).unwrap();
    assert!(matches!(&events[..], [SessionEvent::Note { text }] if text == summary));
    let notes = notes.lock().unwrap();
    assert_eq!(notes.get().summaries, [summary]);
    assert!(
        notes
            .render_block()
            .unwrap()
            .contains("unverified model notes")
    );
}

#[test]
fn tool_only_compaction_needs_storage_but_no_model_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = state();
    state.working = vec![
        ChatMessage::tool_result("c1", "original tool output"),
        ChatMessage::user("recent task"),
    ];
    let notes = Mutex::new(NotesStore::default());
    let outcome = compact::compact(&state.working, 1, tmp.path()).unwrap();
    assert!(outcome.summarize_input.is_empty());
    commit_compaction(&mut state, &notes, &mut None, outcome, None).unwrap();
    assert!(matches!(
        &state.working[0],
        ChatMessage::Tool { content, .. } if content.starts_with("[harness:elided; full:")
    ));
    assert!(notes.lock().unwrap().is_empty());
}

#[test]
fn failed_eager_elision_keeps_retry_marks_until_a_successful_spill() {
    let tmp = tempfile::tempdir().unwrap();
    let blocked = tmp.path().join("not-a-directory");
    std::fs::write(&blocked, "occupied").unwrap();
    let mut messages = vec![ChatMessage::tool_result(
        "c1",
        "[harness:tool-output id=x]\noriginal output",
    )];
    let before = serde_json::to_value(&messages).unwrap();
    let mut store = NotesStore::default();
    store.mark_droppable(&["[harness:tool-output id=x]".into()]);
    let notes = Mutex::new(store);
    assert!(elide_marked_outputs(&mut messages, &notes, &blocked).is_err());
    assert_eq!(serde_json::to_value(&messages).unwrap(), before);
    assert!(notes.lock().unwrap().droppable_ids().contains("x"));
    assert_eq!(
        elide_marked_outputs(&mut messages, &notes, tmp.path()).unwrap(),
        1
    );
    assert!(notes.lock().unwrap().droppable_ids().is_empty());
}
