use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;

#[derive(Clone, Default)]
pub(crate) struct Script {
    /// SSE bodies served in order; the last one repeats forever.
    responses: Arc<StdMutex<Vec<String>>>,
    /// Raw request bodies received, in order.
    requests: Arc<StdMutex<Vec<String>>>,
    sub_requests: Arc<AtomicUsize>,
}

impl Script {
    pub(crate) fn push(&self, sse_body: impl Into<String>) {
        self.responses.lock().unwrap().push(sse_body.into());
    }

    pub(crate) fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    pub(crate) fn requests_snapshot(&self) -> Vec<String> {
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

    if is_sub_request {
        // First sub round -> glob tool call. Second -> summary answer.
        let is_round_two = script.sub_requests.fetch_add(1, Ordering::SeqCst) >= 1;
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
    build_stream_response(next)
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
pub(crate) async fn serve(script: Script) -> String {
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
