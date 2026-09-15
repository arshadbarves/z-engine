use std::io::Write;

use super::*;
use crate::verification::{TASK_REPORT_SCHEMA_VERSION, TaskReport, TaskStatus};

fn sample_events() -> Vec<SessionEvent> {
    vec![
        SessionEvent::Meta {
            model: "m".into(),
            project_root: ".".into(),
        },
        SessionEvent::UserMsg {
            text: "fix it".into(),
            images: vec![],
        },
        SessionEvent::AssistantMsg {
            content: Some("looking".into()),
            tool_calls: vec![PersistedToolCall {
                id: "c1".into(),
                name: "read_file".into(),
                arguments: r#"{"path":"a.txt"}"#.into(),
            }],
        },
        SessionEvent::ToolResult {
            tool_call_id: "c1".into(),
            content: "contents".into(),
        },
        SessionEvent::Note {
            text: "FACTS: something".into(),
        },
    ]
}

pub(super) fn report_event() -> SessionEvent {
    SessionEvent::TaskUpdated {
        report: Box::new(TaskReport {
            schema_version: TASK_REPORT_SCHEMA_VERSION,
            task_id: "task-1".into(),
            goal: "fix it".into(),
            workspace_root: ".".into(),
            status: TaskStatus::Complete,
            requirements: vec![],
            checks: vec![],
            assessment: None,
            blockers: vec![],
            changed_paths: vec![],
            supervision: None,
        }),
    }
}

fn append_raw(path: &std::path::Path, suffix: &[u8]) {
    let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(suffix).unwrap();
    file.flush().unwrap();
}

#[test]
fn roundtrip_preserves_events() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut writer = SessionWriter::create(dir.path()).unwrap();
    for event in sample_events() {
        writer.record(&event).unwrap();
    }
    assert_eq!(read_events(&writer.path).unwrap(), sample_events());
}

#[test]
fn durable_task_updated_roundtrip_preserves_camel_case_report() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut writer = SessionWriter::create(dir.path()).unwrap();
    let event = report_event();
    writer.record_durable(&event).unwrap();
    assert_eq!(read_events(&writer.path).unwrap(), vec![event.clone()]);
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "task_updated");
    assert_eq!(json["report"]["schemaVersion"], 1);
    assert_eq!(json["report"]["taskId"], "task-1");
    assert_eq!(json["report"]["workspaceRoot"], ".");
}

#[test]
fn torn_final_line_is_skipped_but_invalidates_completion() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut writer = SessionWriter::create(dir.path()).unwrap();
    writer.record(&sample_events()[1]).unwrap();
    writer.record_durable(&report_event()).unwrap();
    append_raw(&writer.path, br#"{"type":"tool_result","content":"trunca"#);
    let read = read_events(&writer.path).unwrap();
    assert_eq!(read.len(), 2);
    let SessionEvent::TaskUpdated { report } = &read[1] else {
        panic!("report expected")
    };
    assert_eq!(report.status, TaskStatus::Interrupted);
    assert!(!report.blockers.is_empty());
}

#[test]
fn recovery_persists_downgrade_and_keeps_older_append_handles_current() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut original = SessionWriter::create(dir.path()).unwrap();
    original.record_durable(&report_event()).unwrap();
    append_raw(&original.path, br#"{"type":"tool_result""#);
    let mut resumed = SessionWriter::append_to(&original.path).unwrap();
    resumed.record_durable(&SessionEvent::Ack).unwrap();
    original
        .record(&SessionEvent::Title {
            text: "Recovered".into(),
        })
        .unwrap();
    let events = read_events(&original.path).unwrap();
    assert_eq!(events.len(), 3);
    let SessionEvent::TaskUpdated { report } = &events[0] else {
        panic!("report expected")
    };
    assert_eq!(report.status, TaskStatus::Interrupted);
}

#[test]
fn torn_legacy_transcript_replays_without_inventing_a_task_report() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut writer = SessionWriter::create(dir.path()).unwrap();
    for event in sample_events() {
        writer.record(&event).unwrap();
    }
    append_raw(
        &writer.path,
        br#"{"type":"assistant_msg","content":"unfinished"#,
    );
    let events = read_events(&writer.path).unwrap();
    assert_eq!(events, sample_events());
    assert_eq!(replay(&events).working.len(), 3);
}

#[test]
fn malformed_interior_unknown_suffix_and_unsupported_schema_are_errors() {
    for suffix in [
        b"{bad}\n{\"type\":\"ack\"}\n".as_slice(),
        b"{\"type\":\"future_event\"}\n",
        b"{bad}",
        b"{\"type\":\"user_msg\"}",
        b"{\"type\":\"user_msg\",\"text\":\"torn\n",
    ] {
        let dir = tempfile::tempdir_in(".").unwrap();
        let mut writer = SessionWriter::create(dir.path()).unwrap();
        writer.record_durable(&report_event()).unwrap();
        append_raw(&writer.path, suffix);
        assert_eq!(
            read_events(&writer.path).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }
    let dir = tempfile::tempdir_in(".").unwrap();
    let writer = SessionWriter::create(dir.path()).unwrap();
    let mut json = serde_json::to_value(report_event()).unwrap();
    json["report"]["schemaVersion"] = 999.into();
    append_raw(
        &writer.path,
        serde_json::to_string(&json).unwrap().as_bytes(),
    );
    assert_eq!(
        read_events(&writer.path).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[test]
fn complete_json_without_final_newline_can_be_resumed() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let writer = SessionWriter::create(dir.path()).unwrap();
    append_raw(&writer.path, br#"{"type":"user_msg","text":"legacy"}"#);
    let mut resumed = SessionWriter::append_to(&writer.path).unwrap();
    resumed.record(&SessionEvent::Ack).unwrap();
    assert_eq!(read_events(&writer.path).unwrap().len(), 2);
}

#[test]
fn trimming_keeps_all_append_handles_on_the_new_transcript() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut writer = SessionWriter::create(dir.path()).unwrap();
    let mut title = SessionWriter::append_to(&writer.path).unwrap();
    for event in sample_events() {
        writer.record(&event).unwrap();
    }
    trim_file_before_user_turn(&writer.path, 0).unwrap();
    writer.record(&sample_events()[1]).unwrap();
    title
        .record(&SessionEvent::Title {
            text: "trimmed".into(),
        })
        .unwrap();
    let events = read_events(&writer.path).unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], SessionEvent::Meta { .. }));
    assert!(matches!(events[2], SessionEvent::Title { .. }));
}

#[test]
fn replay_maps_to_chat_messages_and_notes_ignoring_reports() {
    let mut events = sample_events();
    events.push(report_event());
    let replayed = replay(&events);
    assert_eq!(replayed.working.len(), 3);
    assert!(matches!(
        &replayed.working[1],
        z_engine_provider::ChatMessage::Assistant { tool_calls, .. } if tool_calls.len() == 1
    ));
    assert_eq!(replayed.notes_replayed, vec!["FACTS: something"]);
}

#[test]
fn replay_drops_trailing_orphaned_tool_round() {
    let events = &sample_events()[1..3];
    let replayed = replay(events);
    assert_eq!(replayed.working.len(), 1);
    assert!(matches!(
        &replayed.working[0],
        z_engine_provider::ChatMessage::User { .. }
    ));
}

#[test]
fn list_sessions_orders_newest_first_and_previews() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let mut older = SessionWriter::create(dir.path()).unwrap();
    older
        .record(&SessionEvent::UserMsg {
            text: "old task".into(),
            images: vec![],
        })
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let mut newer = SessionWriter::create(dir.path()).unwrap();
    newer
        .record(&SessionEvent::UserMsg {
            text: "new task".into(),
            images: vec![],
        })
        .unwrap();
    let list = list_sessions(dir.path());
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].first_user_msg.as_deref(), Some("new task"));
    assert_eq!(list[1].first_user_msg.as_deref(), Some("old task"));
}
