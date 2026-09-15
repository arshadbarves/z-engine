use super::*;
use std::path::Path;

#[test]
fn roots_match_ignores_slash_and_case() {
    assert!(roots_match("/tmp/proj", Path::new("/tmp/proj/")));
    assert!(roots_match("/tmp/Proj", Path::new("/tmp/proj")));
    assert!(!roots_match("/tmp/proj", Path::new("/tmp/other")));
}

fn task(status: TaskStatus) -> SessionEvent {
    SessionEvent::TaskUpdated {
        report: Box::new(TaskReport {
            schema_version: z_engine_core::verification::TASK_REPORT_SCHEMA_VERSION,
            task_id: "task-1".into(),
            goal: "test replay".into(),
            workspace_root: ".".into(),
            status,
            requirements: vec![z_engine_core::verification::Requirement {
                id: "r1".into(),
                description: "preserve replay state".into(),
            }],
            checks: vec![],
            assessment: None,
            blockers: vec![],
            changed_paths: vec![],
            supervision: None,
        }),
    }
}

fn meta(root: &str) -> SessionEvent {
    SessionEvent::Meta {
        model: "m".into(),
        project_root: root.into(),
    }
}

fn replay_in_workspace(mut events: Vec<SessionEvent>, is_live: bool) -> Vec<serde_json::Value> {
    struct Workspace(PathBuf);
    impl Drop for Workspace {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = Workspace(PathBuf::from(format!(
        ".session-replay-{}-{nonce}",
        std::process::id()
    )));
    std::fs::create_dir(&workspace.0).unwrap();
    std::fs::write(
        workspace.0.join("Cargo.toml"),
        "[package]\nname = \"replay-fixture\"\nversion = \"0.1.0\"\n[workspace]\n",
    )
    .unwrap();
    let root = std::fs::canonicalize(&workspace.0)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    for event in &mut events {
        match event {
            SessionEvent::Meta { project_root, .. } => *project_root = root.clone(),
            SessionEvent::TaskUpdated { report } => report.workspace_root = root.clone(),
            _ => {}
        }
    }
    session_events_json(&events, is_live).unwrap()
}

#[test]
fn legacy_events_are_not_reinterpreted_as_verified() {
    let events = vec![
        meta("."),
        SessionEvent::TurnEnd {
            outcome: "completed".into(),
        },
    ];
    for is_live in [false, true] {
        let json = session_events_json(&events, is_live).unwrap();
        assert_eq!(
            json,
            events
                .iter()
                .map(|event| serde_json::to_value(event).unwrap())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn only_latest_report_is_revalidated_without_promoting_old_completion() {
    let events = vec![
        meta("."),
        task(TaskStatus::Complete),
        task(TaskStatus::Running),
    ];
    let json = replay_in_workspace(events, false);
    assert_eq!(json[1]["report"]["status"], "complete");
    assert_eq!(json[2]["report"]["status"], "interrupted");
    assert!(!json[2]["report"]["blockers"].as_array().unwrap().is_empty());
}

#[test]
fn unfinished_states_only_become_interrupted_after_restart() {
    for (status, live_status) in [
        (TaskStatus::Running, "running"),
        (TaskStatus::NeedsVerification, "needs_verification"),
    ] {
        for is_live in [false, true] {
            let json = replay_in_workspace(vec![meta("."), task(status)], is_live);
            assert_eq!(
                json[1]["report"]["status"],
                if is_live { live_status } else { "interrupted" }
            );
            assert_eq!(
                json[1]["report"]["blockers"].as_array().unwrap().is_empty(),
                is_live
            );
        }
    }
}

#[test]
fn live_loop_does_not_resurrect_an_older_tasks_unfinished_state() {
    let mut current = task(TaskStatus::Running);
    let SessionEvent::TaskUpdated { report } = &mut current else {
        unreachable!()
    };
    report.task_id = "task-2".into();
    let events = vec![meta("."), task(TaskStatus::Running), current];
    let json = replay_in_workspace(events, true);
    assert_eq!(json[1]["report"]["status"], "interrupted");
    assert_eq!(json[2]["report"]["status"], "running");
}

#[test]
fn new_user_message_prevents_preserving_the_previous_tasks_running_state() {
    let json = replay_in_workspace(
        vec![
            meta("."),
            task(TaskStatus::Running),
            SessionEvent::UserMsg {
                text: "next task".into(),
                images: vec![],
            },
        ],
        true,
    );
    assert_eq!(json[1]["report"]["status"], "interrupted");
}

#[test]
fn live_sidebar_preserves_active_status_without_overriding_ack() {
    for (status, outcome) in [
        (TaskStatus::Running, "running"),
        (TaskStatus::NeedsVerification, "needs_verification"),
    ] {
        let mut events = vec![
            meta("."),
            task(status),
            SessionEvent::TurnEnd {
                outcome: "completed".into(),
            },
        ];
        assert_eq!(
            projected_unread_outcome(&events, true).as_deref(),
            Some(outcome)
        );
        assert_eq!(
            projected_unread_outcome(&events, false).as_deref(),
            Some("interrupted")
        );
        events.push(SessionEvent::Ack);
        assert_eq!(projected_unread_outcome(&events, true), None);
    }
}

#[test]
fn missing_or_mismatched_meta_boundary_cannot_replay_complete() {
    for is_live in [false, true] {
        for events in [
            vec![task(TaskStatus::Complete)],
            vec![meta(".."), task(TaskStatus::Complete)],
        ] {
            let json = session_events_json(&events, is_live).unwrap();
            let report = &json.last().unwrap()["report"];
            assert_eq!(report["status"], "stale");
            assert!(!report["blockers"].as_array().unwrap().is_empty());
        }
    }
}

#[test]
fn live_reattach_still_revalidates_completed_evidence() {
    let json = replay_in_workspace(vec![meta("."), task(TaskStatus::Complete)], true);
    assert_ne!(json[1]["report"]["status"], "complete");
}

#[test]
fn restart_interruption_survives_a_later_live_reattach() {
    struct Transcript(PathBuf);
    impl Drop for Transcript {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).unwrap();
        }
    }
    let mut writer = z_engine_core::session::SessionWriter::create(Path::new(".")).unwrap();
    let transcript = Transcript(writer.path.clone());
    let events = vec![meta("."), task(TaskStatus::Running)];
    for event in &events {
        writer.record_durable(event).unwrap();
    }
    persist_restart_interruptions(&events, &transcript.0).unwrap();
    let recorded = z_engine_core::session::read_events(&transcript.0).unwrap();
    let json = session_events_json(&recorded, true).unwrap();
    let report = &json.last().unwrap()["report"];
    assert_eq!(report["status"], "interrupted");
    assert!(!report["blockers"].as_array().unwrap().is_empty());
    persist_restart_interruptions(&recorded, &transcript.0).unwrap();
    assert_eq!(
        z_engine_core::session::read_events(&transcript.0).unwrap(),
        recorded
    );
}

#[test]
fn refresh_rejects_unsupported_schema_instead_of_preserving_complete() {
    for is_live in [false, true] {
        let mut event = task(TaskStatus::Complete);
        let SessionEvent::TaskUpdated { report } = &mut event else {
            unreachable!()
        };
        report.schema_version = 999;
        let json = session_events_json(&[meta("."), event], is_live).unwrap();
        assert_ne!(json[1]["report"]["status"], "complete");
        assert!(!json[1]["report"]["blockers"].as_array().unwrap().is_empty());
    }
}
