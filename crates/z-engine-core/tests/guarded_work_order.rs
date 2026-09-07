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

/// Invariant 7, enforced rather than observed (finding I3).
///
/// The bounded prompt builder decides what a guarded run sends. When the
/// pinned content alone will not fit the context budget, there is no
/// request to make — so the turn is blocked *before* the provider is
/// called, and the tape shows no round at all.
#[tokio::test]
async fn a_guarded_prompt_that_cannot_be_bounded_blocks_before_the_provider_is_called() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "hi\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        text_delta("this must never be reached."),
        finish_json("stop", 5, 5),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base, tmp.path());
    cfg.guarded = true;
    // L0 alone is thousands of tokens, so a budget of ten cannot hold the
    // pinned content whatever the conversation looks like.
    cfg.max_context_tokens = 10;
    let (handle, mut ev) = spawn(cfg);
    handle.submit("do something");

    let blocked = wait_for(&mut ev, |e| matches!(e, Event::TurnBlocked { .. })).await;
    let Event::TurnBlocked { gate, reason, .. } = blocked else {
        unreachable!()
    };
    assert_eq!(gate, "prompt-budget");
    assert!(reason.contains("pinned content"), "{reason}");
    assert!(reason.contains("nothing was sent"), "{reason}");
    assert!(
        script.requests_snapshot().is_empty(),
        "an unbounded prompt must never reach the provider"
    );
}

/// The other half: what the manifest pins is what the model sees. A
/// guarded run's evidence excerpts are a pinned section, so they have to
/// be on the wire — a manifest that measured content nobody sent would
/// be a description, not a bound.
#[tokio::test]
async fn the_evidence_the_manifest_pins_is_on_the_wire() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "pub fn parse() {}\n").unwrap();

    let script = Script::default();
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
            r#"{"goal":"make parse fallible","writable_paths":["src/lib.rs"],"target_symbols":["parse"],"evidence_ids":["__EVIDENCE_ID__"],"acceptance_commands":[{"command":"cargo test","description":"unit tests"}]}"#
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
    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;

    let bodies = script.requests_snapshot();
    assert_eq!(bodies.len(), 3, "expected three model rounds");
    assert!(
        bodies[2].contains("Evidence backing the active work order"),
        "the pinned evidence section must reach the model"
    );
    assert!(
        bodies[2].contains("working-tree"),
        "the excerpt names the revision it was read at"
    );
    // …and an unguarded run gains none of it.
    assert!(!bodies[0].contains("Evidence backing the active work order"));
}
