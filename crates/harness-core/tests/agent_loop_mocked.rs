//! Integration tests: the full agent loop against a mocked
//! OpenAI-compatible provider serving canned SSE (spec §10).
//!
//! The mock is an axum app with a scripted queue of responses; every
//! request body is captured so tests can assert on what the model received.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use harness_core::agent::{spawn, Event, LoopConfig};

// ---------------------------------------------------------------------------
// Mock provider infrastructure
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Script {
    /// SSE bodies served in order; the last one repeats forever.
    responses: Arc<StdMutex<Vec<String>>>,
    /// Raw request bodies received, in order.
    requests: Arc<StdMutex<Vec<String>>>,
}

impl Script {
    fn push(&self, sse_body: impl Into<String>) {
        self.responses.lock().unwrap().push(sse_body.into());
    }

    fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn requests_snapshot(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn chat_handler(State(script): State<Script>, req: axum::extract::Request) -> Response {
    let bytes = axum::body::to_bytes(req.into_body(), 50_000_000)
        .await
        .unwrap_or_default();
    script
        .requests
        .lock()
        .unwrap()
        .push(String::from_utf8_lossy(&bytes).into_owned());

    let next = {
        let mut q = script.responses.lock().unwrap();
        match q.len() {
            0 => "data: [DONE]\n\n".to_string(),
            1 => q[0].clone(),
            _ => q.remove(0),
        }
    };

    let chunk = axum::body::Bytes::from(next);
    let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(chunk) });
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// Serve the script on a random localhost port; returns its base_url.
async fn serve(script: Script) -> String {
    let app = axum::Router::new()
        .route("/chat/completions", post(chat_handler))
        .with_state(script);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn cfg_for(base_url: String, project_root: &std::path::Path) -> LoopConfig {
    LoopConfig {
        model: "test-model".into(),
        base_url,
        api_key: Some("test-key-not-real".into()),
        project_root: project_root.to_path_buf(),
        tmp_dir: project_root.join("tmp-out"),
        initial_allow_rules: vec!["echo*".to_string()],
    }
}

// ---------------------------------------------------------------------------
// SSE builders
// ---------------------------------------------------------------------------

fn sse_event(json: &str) -> String {
    format!("data: {json}\n\n")
}

fn text_delta(t: &str) -> String {
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"content":"{t}"}}}}]}}"#
    ))
}

fn finish_json(reason: &str, prompt: u64, completion: u64) -> String {
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{}},"finish_reason":"{reason}"}}],"usage":{{"prompt_tokens":{prompt},"completion_tokens":{completion}}}}}"#
    ))
}

/// One tool-call delta fragment. `args` is raw JSON text (may be a partial).
fn tool_call_delta(index: usize, id: Option<&str>, name: Option<&str>, args: &str) -> String {
    let escaped = args.replace('\\', "\\\\").replace('"', "\\\"");
    let id_part = id.map(|i| format!(r#""id":"{i}","#)).unwrap_or_default();
    let fn_name = name.map(|n| format!(r#""name":"{n}","#)).unwrap_or_default();
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":{index},{id_part}"type":"function","function":{{{fn_name}"arguments":"{escaped}"}}}}]}}}}]}}"#
    ))
}

fn done() -> String {
    "data: [DONE]\n\n".to_string()
}

/// Drain events until `pred` matches or a deadline passes.
async fn wait_for(ev: &mut harness_core::agent::EventRx, pred: impl Fn(&Event) -> bool) -> Event {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting for event");
        let e = tokio::time::timeout(Duration::from_millis(500), ev.recv())
            .await
            .ok()
            .flatten();
        match e {
            Some(e) if pred(&e) => return e,
            Some(_) => continue,
            None => continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_loop_read_then_bash_then_answer() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("notes.txt"), "the secret number is 41\n").unwrap();

    let script = Script::default();
    // R1: read a file · R2: allowed bash · R3: final answer.
    script.push(format!(
        "{}{}{}{}",
        text_delta("Let me look."),
        tool_call_delta(0, Some("call_read"), Some("read_file"), r#"{"path":"notes.txt"}"#),
        finish_json("tool_calls", 100, 20),
        done()
    ));
    script.push(format!(
        "{}{}{}{}",
        text_delta("Now incrementing."),
        tool_call_delta(0, Some("call_bash"), Some("bash"), r#"{"command":"echo 42 > out.txt"}"#),
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

    let finished = wait_for(&mut ev, |e| {
        matches!(e, Event::ToolCallFinished { name, .. } if name == "read_file")
    })
    .await;
    let Event::ToolCallFinished { ok, summary, .. } = finished else { unreachable!() };
    assert!(ok, "{summary}");
    assert!(summary.contains("notes.txt"));

    // `echo*` was pre-allowed → bash runs without any ApprovalRequired.
    let bash_done = wait_for(&mut ev, |e| {
        matches!(e, Event::ToolCallFinished { name, .. } if name == "bash")
    })
    .await;
    assert!(matches!(bash_done, Event::ToolCallFinished { ok: true, .. }));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    let Event::TurnCompleted { prompt_tokens, completion_tokens } = completed else {
        unreachable!()
    };
    assert_eq!(prompt_tokens, 300); // latest prompt size wins
    assert_eq!(completion_tokens, 120); // 20 + 40 + 60 cumulative

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("out.txt")).unwrap().trim(),
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
    script.push(format!("{}{}{}", text_delta("got it"), finish_json("stop", 20, 20), done()));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("read fact");

    let _ = wait_for(&mut ev, |e| matches!(e, Event::ToolCallFinished { ok: true, .. })).await;
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
async fn gated_bash_prompt_then_deny_refuses_and_model_adapts() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("rm time"),
        tool_call_delta(0, Some("c1"), Some("bash"), r#"{"command":"rm dangerous-thing"}"#),
        finish_json("tool_calls", 5, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("Understood, adjusting."),
        finish_json("stop", 6, 6),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("clean up");

    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired { id, tool, input_preview, suggested_rule } = approval else {
        unreachable!()
    };
    assert_eq!(tool, "bash");
    assert!(input_preview.contains("dangerous"));
    assert_eq!(suggested_rule.as_deref(), Some("rm dangerous-thing*"));

    handle.deny(id);

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

#[tokio::test]
async fn approve_always_prefix_skips_second_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // Two consecutive identical gated commands, then a closing answer.
    script.push(format!(
        "{}{}{}",
        tool_call_delta(0, Some("c0"), Some("bash"), r#"{"command":"cargo test all"}"#),
        finish_json("tool_calls", 9, 9),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        tool_call_delta(0, Some("c1"), Some("bash"), r#"{"command":"cargo test all"}"#),
        finish_json("tool_calls", 9, 9),
        done()
    ));
    script.push(format!("{}{}{}", text_delta("all green"), finish_json("stop", 9, 9), done()));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("run tests");

    // First prompt → answer "always this prefix".
    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired { id, suggested_rule, .. } = approval else {
        unreachable!()
    };
    assert_eq!(suggested_rule.as_deref(), Some("cargo test*"));
    handle.approve(id, suggested_rule.clone());

    // Second identical command must NOT re-prompt: straight to completion.
    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

#[tokio::test]
async fn abort_mid_stream_ends_turn_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // A very long single response: enough drips to still be streaming.
    let long_body =
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"tick\"}}]}\n\n".repeat(200_000);
    script.push(long_body);

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("count ticks");

    let _ = wait_for(&mut ev, |e| matches!(e, Event::TokenDelta(_))).await;
    handle.abort();

    let aborted = wait_for(&mut ev, |e| matches!(e, Event::TurnAborted)).await;
    assert!(matches!(aborted, Event::TurnAborted));
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
    script.push(format!("{}{}{}", text_delta("both read"), finish_json("stop", 8, 8), done()));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("read both");

    let finished_a = wait_for(&mut ev, |e| matches!(e, Event::ToolCallStarted { .. })).await;
    assert!(matches!(finished_a, Event::ToolCallStarted { .. }));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

#[tokio::test]
async fn shutdown_stops_the_task() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    script.push(format!("{}{}{}", text_delta("hi"), finish_json("stop", 1, 1), done()));
    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.shutdown();
    // Once the task exits, the event channel closes (recv → None forever).
    loop {
        match ev.recv().await {
            Some(_) => continue,
            None => break,
        }
    }
}
