//! Forced-process termination and replay use only a private Rust test subprocess.

#[path = "support/desktop_retirement_fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::time::Duration;

use fixture::{Provider, answer, config, next_event, shutdown, tool};
use serde_json::json;
use z_engine_core::agent::{ApprovalDecision, Event, ResumeState, spawn_with_recorder};
use z_engine_core::session::{SessionEvent, SessionWriter, read_events, replay};

const WORKER_ROOT: &str = "ZENGINE_DESKTOP_CRASH_FIXTURE_ROOT";
const GOAL: &str = "Record the phoenix marker, then continue investigating.";
const MARKER: &str = "PHOENIX_MARKER_ZX99";
const SHELL_COMMAND: &str = "echo PHOENIX_MARKER_ZX99>>proof.txt";

#[tokio::test]
async fn forced_exit_preserves_tool_context_without_repeating_shell_effects() {
    let directory = tempfile::Builder::new()
        .prefix(".desktop-recovery-")
        .tempdir_in(".")
        .unwrap();
    let root = directory.path().canonicalize().unwrap();
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = root.join("session.jsonl");
    let mut worker = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "desktop_crash_worker",
            "--ignored",
            "--nocapture",
        ])
        .env(WORKER_ROOT, &root)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        while !root.join("ready").exists() {
            assert!(
                worker.try_wait().unwrap().is_none(),
                "crash fixture exited early"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("worker never reached the stalled model request");
    worker.start_kill().unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), worker.wait())
        .await
        .expect("worker was not reaped")
        .unwrap();
    assert!(!status.success());
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(9));
    }

    let before = read_events(&session).unwrap();
    assert!(
        before
            .iter()
            .any(|event| { matches!(event, SessionEvent::UserMsg { text, .. } if text == GOAL) })
    );
    assert!(before.iter().any(|event| {
        matches!(event, SessionEvent::ToolResult { tool_call_id, content }
            if tool_call_id == "read-proof" && content.contains(MARKER))
    }));
    assert!(
        !before
            .iter()
            .any(|event| matches!(event, SessionEvent::TurnEnd { .. }))
    );
    let proof_before = std::fs::read_to_string(workspace.join("proof.txt")).unwrap();
    assert_eq!(proof_before.lines().collect::<Vec<_>>(), [MARKER]);

    let replayed = replay(&before);
    let provider = Provider::start(vec![answer("Resumed with the recorded phoenix marker.")]).await;
    let (handle, mut events) = spawn_with_recorder(
        config(&provider, &workspace),
        Some(ResumeState {
            working: replayed.working,
            note_payloads: replayed.notes_replayed,
        }),
        Some(SessionWriter::append_to(&session).unwrap()),
    );
    handle.submit("continue");
    let mut response = String::new();
    loop {
        match next_event(&mut events).await {
            Event::TokenDelta(text) => response.push_str(&text),
            Event::ApprovalRequired { .. } | Event::ToolCallStarted { .. } => {
                panic!("replay must not re-execute recorded tool calls");
            }
            Event::TurnCompleted { .. } => break,
            _ => {}
        }
    }
    shutdown(handle, events).await;
    provider.assert_consumed();
    assert!(response.contains("Resumed with the recorded phoenix marker"));
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let messages = requests[0]["messages"].as_array().unwrap();
    let (newest, history) = messages.split_last().unwrap();
    assert!(
        history
            .iter()
            .any(|message| { message["role"] == "user" && message["content"] == GOAL })
    );
    assert!(
        history
            .iter()
            .any(|message| { message["role"] == "user" && message["content"] == "continue" })
    );
    assert_eq!(newest["role"], "user");
    let packet: serde_json::Value =
        serde_json::from_str(newest["content"].as_str().unwrap()).unwrap();
    assert_eq!(packet["kind"], "task_context");
    assert_eq!(packet["harness"]["originalGoal"], "continue");
    assert!(messages.iter().any(|message| {
        message["role"] == "tool"
            && message["tool_call_id"] == "read-proof"
            && message["content"].as_str().unwrap().contains(MARKER)
    }));
    assert_eq!(
        std::fs::read_to_string(workspace.join("proof.txt")).unwrap(),
        proof_before
    );
    let after = read_events(&session).unwrap();
    assert!(after.starts_with(&before));
    assert!(after.iter().any(|event| {
        matches!(event, SessionEvent::UserMsg { text, .. } if text == "continue")
    }));
    assert!(!after.iter().any(|event| {
        matches!(event, SessionEvent::UserMsg { text, .. }
            if text == newest["content"].as_str().unwrap())
    }));
    assert!(
        after
            .iter()
            .any(|event| matches!(event, SessionEvent::TurnEnd { .. }))
    );
}

#[tokio::test]
#[ignore = "private subprocess fixture, launched by forced_exit_preserves_tool_context_without_repeating_shell_effects"]
async fn desktop_crash_worker() {
    let root = PathBuf::from(std::env::var_os(WORKER_ROOT).expect("missing private fixture root"));
    let workspace = root.join("workspace");
    let provider = Provider::start(vec![
        tool("shell-proof", "bash", json!({"command": SHELL_COMMAND})),
        tool("read-proof", "read_file", json!({"path": "proof.txt"})),
        None,
    ])
    .await;
    let recorder = SessionWriter::append_to(&root.join("session.jsonl")).unwrap();
    let (handle, mut events) =
        spawn_with_recorder(config(&provider, &workspace), None, Some(recorder));
    handle.submit(GOAL);
    let mut approvals = 0;
    loop {
        match next_event(&mut events).await {
            Event::ApprovalRequired {
                id,
                tool,
                bash_command,
                ..
            } => {
                assert_eq!(tool, "bash");
                assert_eq!(bash_command.as_deref(), Some(SHELL_COMMAND));
                assert!(!workspace.join("proof.txt").exists());
                approvals += 1;
                handle.approve(id, ApprovalDecision::Once);
            }
            Event::ToolCallFinished {
                name, ok, summary, ..
            } => {
                assert!(ok, "{summary}");
                if name == "read_file" {
                    break;
                }
            }
            Event::TurnCompleted { .. } => panic!("worker completed before its stalled request"),
            _ => {}
        }
    }
    assert_eq!(approvals, 1);
    tokio::time::timeout(Duration::from_secs(10), async {
        while provider.requests().len() < 3 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("agent did not send the follow-up request");
    provider.assert_consumed();
    let transcript = read_events(&root.join("session.jsonl")).unwrap();
    assert!(transcript.iter().any(|event| {
        matches!(event, SessionEvent::ToolResult { tool_call_id, content }
            if tool_call_id == "read-proof" && content.contains(MARKER))
    }));
    std::fs::write(root.join("ready"), b"waiting for forced termination").unwrap();
    // Keep the recorder, agent and provider alive until the parent forcibly kills us.
    loop {
        let event = next_event(&mut events).await;
        assert!(
            !matches!(
                event,
                Event::TurnCompleted { .. }
                    | Event::ApprovalRequired { .. }
                    | Event::ToolCallStarted { .. }
            ),
            "unexpected event while waiting for forced termination: {event:?}"
        );
    }
}
