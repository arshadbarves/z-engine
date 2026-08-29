//! Guarded-mode integration tests (Task 4): a work order declared through
//! `set_work_order` must be admitted only on fresh evidence and must reach
//! the next request's prompt — while unguarded runs stay untouched.

mod common;

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for};
use z_engine_core::agent::{Event, spawn};

/// Guarded mode (opt-in, Task 4): a work order set through `set_work_order`
/// must reach the *next* request's prompt, and must only be admitted when
/// each writable path is backed by fresh read evidence recorded this run.
#[tokio::test]
async fn guarded_work_order_reaches_the_next_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "pub fn parse() {}\n").unwrap();

    let script = Script::default();
    // R1: read the file (records evidence) · R2: declare the work order,
    // citing the evidence id the harness just handed back · R3: answer.
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("call_read"),
            Some("read_file"),
            r#"{"path":"./src/lib.rs"}"#
        ),
        finish_json("tool_calls", 10, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("call_order"),
            Some("set_work_order"),
            r#"{"goal":"make parse fallible","writable_paths":["src/../src/lib.rs"],"target_symbols":["parse"],"evidence_ids":["__EVIDENCE_ID__"],"acceptance_commands":[{"command":"cargo test","description":"unit tests"}]}"#
        ),
        finish_json("tool_calls", 20, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("order accepted."),
        finish_json("stop", 30, 5),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base, tmp.path());
    cfg.guarded = true;
    let (handle, mut ev) = spawn(cfg);
    handle.submit("plan the parse change");

    let done_order = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "set_work_order"),
    )
    .await;
    let Event::ToolCallFinished { ok, summary, .. } = done_order else {
        unreachable!()
    };
    assert!(ok, "work order rejected: {summary}");

    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;

    let bodies = script.requests_snapshot();
    assert_eq!(bodies.len(), 3, "expected three model rounds");
    // Guarded runs advertise the governance tool…
    assert!(
        bodies[0].contains(r#""name":"set_work_order""#),
        "guarded run must advertise set_work_order"
    );
    // …and the accepted order is pinned into the following prompt, with the
    // model's `src/../src/lib.rs` spelling normalized to repo-relative form.
    let at = bodies[2]
        .find("# Active work order")
        .expect("work order digest missing from the next prompt");
    let digest = &bodies[2][at..at + 400.min(bodies[2].len() - at)];
    assert!(
        digest.contains("make parse fallible") && digest.contains("src/lib.rs"),
        "work order digest lost its goal/path: {digest}"
    );
    assert!(
        !digest.contains("src/../"),
        "writable path was not normalized at the boundary: {digest}"
    );
}

/// Unguarded runs (the default) must be untouched: no governance tool is
/// advertised and no work-order digest is ever pinned into the prompt.
#[tokio::test]
async fn unguarded_runs_never_see_work_order_machinery() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "hi\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        text_delta("nothing to do."),
        finish_json("stop", 5, 5),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("hello");
    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;

    let bodies = script.requests_snapshot();
    assert!(
        !bodies[0].contains(r#""name":"set_work_order""#),
        "unguarded run must not advertise the governance tool"
    );
    assert!(!bodies[0].contains("# Active work order"));
}
