//! Shared mock-provider harness for the agent-loop integration tests.
//!
//! The mock is an axum app with a scripted queue of SSE responses; every
//! request body is captured so tests can assert on what the model
//! received. Lives here so each scenario file stays about its scenarios.
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use z_engine_core::agent::{Event, LoopConfig};

// ---------------------------------------------------------------------------
// Mock provider infrastructure
// ---------------------------------------------------------------------------

static SUB_REQUESTS: AtomicUsize = AtomicUsize::new(0);

/// Sub-agent request counter, shared by the mock handler across tests in
/// one binary; reset it when a test asserts on sub-agent scripting.
pub fn reset_sub_requests() {
    SUB_REQUESTS.store(0, Ordering::SeqCst);
}

#[derive(Clone, Default)]
pub struct Script {
    /// SSE bodies served in order; the last one repeats forever.
    responses: Arc<StdMutex<Vec<String>>>,
    /// Raw request bodies received, in order.
    pub requests: Arc<StdMutex<Vec<String>>>,
}

impl Script {
    pub fn push(&self, sse_body: impl Into<String>) {
        self.responses.lock().unwrap().push(sse_body.into());
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    pub fn requests_snapshot(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn chat_handler(State(script): State<Script>, req: axum::extract::Request) -> Response {
    let bytes = axum::body::to_bytes(req.into_body(), 50_000_000)
        .await
        .unwrap_or_default();
    let body_text = String::from_utf8_lossy(&bytes).into_owned();
    let is_title_request = body_text.contains("Reply with a session title only");
    let is_sub_request = body_text.contains("research sub-agent");
    eprintln!(
        "[DBG-MOCK] classified sub={} summarizer={} title={} len={} head={}",
        body_text.contains("research sub-agent"),
        body_text.contains("compress an earlier portion"),
        is_title_request,
        body_text.len(),
        &body_text[..body_text.len().min(120)]
    );
    if is_title_request {
        let body = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "Mock Session Title"},
                "finish_reason": "stop"
            }]
        });
        return build_stream_response(format!("data: {body}\n\ndata: [DONE]\n\n"));
    }
    script.requests.lock().unwrap().push(body_text.clone());

    let sub_count = if is_sub_request {
        SUB_REQUESTS.fetch_add(1, Ordering::SeqCst)
    } else {
        SUB_REQUESTS.load(Ordering::SeqCst)
    };
    if is_sub_request {
        // First sub round -> glob tool call. Second -> summary answer.
        let is_round_two = sub_count >= 1;
        let body = if is_round_two {
            serde_json::json!({
                "choices": [{
                    "index": 0,
                    "delta": {"content": "SUB_SUMMARY_FACTS: found INTERMEDIATE files"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 500, "completion_tokens": 20}
            })
        } else {
            serde_json::json!({
                "choices": [{
                    "index": 0,
                    "delta": {"tool_calls": [{
                        "index": 0,
                        "id": "sg1",
                        "type": "function",
                        "function": {"name": "glob", "arguments": "{\"pattern\":\"*.rs\"}"}
                    }]},
                    "finish_reason": "tool_calls"
                }]
            })
        };
        return build_stream_response(format!("data: {}\n\ndata: [DONE]\n\n", body));
    }

    // Reviewer side-requests get scripted verdicts.
    if body_text.contains("code reviewer") {
        let content = if body_text.contains("NO_FINDINGS_PLEASE") {
            "NO_FINDINGS"
        } else {
            "FINDING: OFF_BY_ONE_RISK in calc.txt"
        };
        let body = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": content},
                "finish_reason": "stop"
            }]
        });
        return build_stream_response(format!("data: {}\n\ndata: [DONE]\n\n", body));
    }

    if String::from_utf8_lossy(&bytes).contains("compress an earlier portion") {
        let body = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "- FACTS: THE_SECRET_ZEBRA_GRAZES_AT_NOON appears early in big.txt; big.txt was ingested three times\n- DECISIONS: none\n- OPEN THREADS: report completion"}
            }]
        });
        return build_stream_response(format!("data: {body}\n\ndata: [DONE]\n\n"));
    }

    let next = {
        let mut q = script.responses.lock().unwrap();
        match q.len() {
            0 => "data: [DONE]\n\n".to_string(),
            1 => q[0].clone(),
            _ => q.remove(0),
        }
    };
    // Evidence ids are minted at runtime, so scripts cite them by
    // placeholder and the mock substitutes the id the harness just handed
    // back in a tool result.
    let next = match latest_evidence_id(&body_text) {
        Some(id) => next.replace("__EVIDENCE_ID__", &id),
        None => next,
    };
    build_stream_response(next)
}

/// Last real `[evidence: <id>]` marker in a captured request body. The
/// system prompt and tool schemas mention the marker with a placeholder,
/// so only alphanumeric ULID-shaped ids count.
pub fn latest_evidence_id(body: &str) -> Option<String> {
    body.match_indices("[evidence: ")
        .filter_map(|(at, marker)| {
            let rest = &body[at + marker.len()..];
            let id = &rest[..rest.find(']')?];
            (id.len() > 10 && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| id.to_string())
        })
        .last()
}

fn build_stream_response(body: String) -> Response {
    if body.contains("sg1") {
        eprintln!("[DBG-MOCK] sg1 body: {body}");
    }
    let chunk = axum::body::Bytes::from(body);
    let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(chunk) });
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// Serve the script on a random localhost port; returns its base_url.
pub async fn serve(script: Script) -> String {
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

pub fn cfg_for(base_url: String, project_root: &std::path::Path) -> LoopConfig {
    LoopConfig {
        model: "test-model".into(),
        base_url,
        api_key: Some("test-key-not-real".into()),
        project_root: project_root.to_path_buf(),
        tmp_dir: project_root.join("tmp-out"),
        initial_allow_rules: vec!["echo*".to_string()],
        max_context_tokens: 100_000,
        max_output_tokens: 16_384,
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        guarded: false,
    }
}

// ---------------------------------------------------------------------------
// SSE builders
// ---------------------------------------------------------------------------

pub fn sse_event(json: &str) -> String {
    format!("data: {json}\n\n")
}

pub fn text_delta(t: &str) -> String {
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"content":"{t}"}}}}]}}"#
    ))
}

pub fn finish_json(reason: &str, prompt: u64, completion: u64) -> String {
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{}},"finish_reason":"{reason}"}}],"usage":{{"prompt_tokens":{prompt},"completion_tokens":{completion}}}}}"#
    ))
}

/// One tool-call delta fragment. `args` is raw JSON text (may be a partial).
pub fn tool_call_delta(index: usize, id: Option<&str>, name: Option<&str>, args: &str) -> String {
    let escaped = args.replace('\\', "\\\\").replace('"', "\\\"");
    let id_part = id.map(|i| format!(r#""id":"{i}","#)).unwrap_or_default();
    let fn_name = name
        .map(|n| format!(r#""name":"{n}","#))
        .unwrap_or_default();
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":{index},{id_part}"type":"function","function":{{{fn_name}"arguments":"{escaped}"}}}}]}}}}]}}"#
    ))
}

pub fn done() -> String {
    "data: [DONE]\n\n".to_string()
}

/// Drain events until `pred` matches or a deadline passes.
pub async fn wait_for(
    ev: &mut z_engine_core::agent::EventRx,
    pred: impl Fn(&Event) -> bool,
) -> Event {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for event"
        );
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
