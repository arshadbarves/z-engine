use std::time::Duration;

use z_engine_core::agent::{Event, spawn};

use crate::mock_loop::{
    Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for,
};

#[tokio::test]
async fn full_loop_read_then_bash_then_answer() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("notes.txt"), "the secret number is 41\n").unwrap();

    let script = Script::default();
    // R1: read a file · R2: allowed bash · R3: final answer.
    script.push(format!(
        "{}{}{}{}",
        text_delta("Let me look."),
        tool_call_delta(
            0,
            Some("call_read"),
            Some("read_file"),
            r#"{"path":"notes.txt"}"#
        ),
        finish_json("tool_calls", 100, 20),
        done()
    ));
    script.push(format!(
        "{}{}{}{}",
        text_delta("Now incrementing."),
        tool_call_delta(
            0,
            Some("call_bash"),
            Some("bash"),
            r#"{"command":"echo 42 > out.txt"}"#
        ),
        finish_json("tool_calls", 200, 40),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("Done — wrote 42."),
        finish_json("stop", 300, 60),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("bump the number");

    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnStarted)).await;

    let finished = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "read_file"),
    )
    .await;
    let Event::ToolCallFinished { ok, summary, .. } = finished else {
        unreachable!()
    };
    assert!(ok, "{summary}");
    assert!(summary.contains("notes.txt"));

    // `echo*` was pre-allowed → bash runs without any ApprovalRequired.
    let bash_done = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "bash"),
    )
    .await;
    assert!(matches!(
        bash_done,
        Event::ToolCallFinished { ok: true, .. }
    ));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    let Event::TurnCompleted {
        prompt_tokens,
        completion_tokens,
    } = completed
    else {
        unreachable!()
    };
    assert_eq!(prompt_tokens, 300); // latest prompt size wins
    assert_eq!(completion_tokens, 120); // 20 + 40 + 60 cumulative

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("out.txt"))
            .unwrap()
            .trim(),
        "42"
    );

    assert_eq!(script.request_count(), 3);
}

#[tokio::test]
async fn tool_result_is_fed_back_to_the_model() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("fact.txt"), "harness-was-here\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("reading"),
        tool_call_delta(0, Some("cr"), Some("read_file"), r#"{"path":"fact.txt"}"#),
        finish_json("tool_calls", 10, 10),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("got it"),
        finish_json("stop", 20, 20),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("read fact");

    let _ = wait_for(&mut ev, |e| {
        matches!(e, Event::ToolCallFinished { ok: true, .. })
    })
    .await;
    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;

    // The second POST must carry the tool result back to the model.
    for _ in 0..50 {
        if script.request_count() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let reqs = script.requests_snapshot();
    assert!(
        reqs.len() >= 2 && reqs[1].contains("harness-was-here"),
        "tool result should be fed back in request #2"
    );
}

#[tokio::test]
async fn malformed_tool_arguments_become_error_result_not_crash() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // Arguments never form valid JSON.
    script.push(format!(
        "{}{}{}",
        tool_call_delta(0, Some("cbad"), Some("read_file"), "{\"path\": "),
        finish_json("tool_calls", 4, 4),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("recovered."),
        finish_json("stop", 5, 5),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("go");

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

#[tokio::test]
async fn parallel_safe_tools_run_in_one_round() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "A\n").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "B\n").unwrap();

    let script = Script::default();
    // Two read_file calls in ONE assistant message (indexes 0 and 1).
    script.push(format!(
        "{}{}{}{}",
        tool_call_delta(0, Some("p1"), Some("read_file"), r#"{"path":"a.txt"}"#),
        tool_call_delta(1, Some("p2"), Some("read_file"), r#"{"path":"b.txt"}"#),
        finish_json("tool_calls", 7, 7),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("both read"),
        finish_json("stop", 8, 8),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("read both");

    let finished_a = wait_for(&mut ev, |e| matches!(e, Event::ToolCallStarted { .. })).await;
    assert!(matches!(finished_a, Event::ToolCallStarted { .. }));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}
