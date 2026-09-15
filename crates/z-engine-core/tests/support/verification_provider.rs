use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use futures::StreamExt;
use serde_json::{Value, json};
use z_engine_core::verification::{CheckEvidence, CheckOutcome};

pub const CANCEL_MARKER: &str = "waiting for cancellation";

pub enum Step {
    Tool {
        id: &'static str,
        name: &'static str,
        input: Value,
    },
    Assess,
    Done,
    Fail,
    WaitForCancellation,
    #[allow(dead_code)]
    MalformedTool,
}

pub fn read() -> Step {
    Step::Tool {
        id: "read-source",
        name: "read_file",
        input: json!({"path": "src/lib.rs"}),
    }
}

pub fn verify(id: &'static str) -> Step {
    Step::Tool {
        id,
        name: "run_verification",
        input: json!({"kind": "cargo_test", "package": null, "filter": null}),
    }
}

pub fn edit(id: &'static str, old: &str, new: &str) -> Step {
    Step::Tool {
        id,
        name: "edit_file",
        input: json!({"path": "src/lib.rs", "old_string": old, "new_string": new}),
    }
}

pub fn repair_steps() -> Vec<Step> {
    vec![
        read(),
        verify("verify-fail"),
        edit("fix-source", "    41\n", "    42\n"),
        verify("verify-pass"),
        Step::Assess,
    ]
}

#[derive(Clone)]
struct Script {
    steps: Arc<Mutex<VecDeque<Step>>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

pub struct Provider {
    pub base_url: String,
    script: Script,
    task: tokio::task::JoinHandle<()>,
}

impl Provider {
    pub async fn start(steps: Vec<Step>) -> Self {
        let script = Script {
            steps: Arc::new(Mutex::new(steps.into())),
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        let router = axum::Router::new()
            .route("/chat/completions", post(handler))
            .with_state(script.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            base_url: format!("http://{address}"),
            script,
            task,
        }
    }

    pub fn assert_consumed(&self) {
        assert!(
            self.script.steps.lock().unwrap().is_empty(),
            "provider scenario did not finish"
        );
    }

    pub fn requests(&self) -> Vec<Value> {
        self.script.requests.lock().unwrap().clone()
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn tool_result<'a>(request: &'a Value, id: &str) -> &'a str {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "tool" && message["tool_call_id"] == id)
        .unwrap_or_else(|| panic!("missing tool result {id}: {request}"))["content"]
        .as_str()
        .unwrap()
}

pub fn check_evidence(request: &Value, id: &str) -> CheckEvidence {
    let output: Value = serde_json::from_str(tool_result(request, id)).unwrap();
    let evidence: CheckEvidence = serde_json::from_value(output["evidence"].clone()).unwrap();
    assert_eq!(output["evidenceId"], evidence.id);
    evidence
}

async fn handler(State(script): State<Script>, request: axum::extract::Request) -> Response {
    let bytes = axum::body::to_bytes(request.into_body(), 2_000_000)
        .await
        .unwrap();
    let request: Value = serde_json::from_slice(&bytes).unwrap();
    let is_title = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| {
            message["role"] == "system"
                && message["content"]
                    .as_str()
                    .is_some_and(|text| text.contains("Reply with a session title only"))
        });
    if is_title {
        return text("Verification fixture");
    }
    script.requests.lock().unwrap().push(request.clone());
    let step = script
        .steps
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected provider request after scenario");
    match step {
        Step::Tool { id, name, input } => tool(id, name, input),
        Step::Assess => {
            let evidence = check_evidence(&request, "verify-pass");
            assert_eq!(evidence.outcome, CheckOutcome::Passed);
            tool(
                "assess-goal",
                "assess_completion",
                json!({
                    "summary": "The answer now matches the original goal.",
                    "coverage": [{
                        "requirementId": "goal",
                        "evidenceIds": [evidence.id],
                        "explanation": "The recorded Cargo check covers the changed answer implementation."
                    }]
                }),
            )
        }
        Step::Done => text("done"),
        Step::MalformedTool => sse(json!({"choices": [{
            "index": 0,
            "delta": {"tool_calls": [{
                "index": 0, "id": "malformed", "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\":"}
            }]},
            "finish_reason": "tool_calls"
        }]})),
        Step::Fail => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("injected provider failure"))
            .unwrap(),
        Step::WaitForCancellation => {
            let chunk = Bytes::from(format!(
                "data: {}\n\n",
                json!({"choices": [{"index": 0, "delta": {"content": CANCEL_MARKER}}]})
            ));
            let stream = futures::stream::once(async { Ok::<_, std::io::Error>(chunk) })
                .chain(futures::stream::pending());
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(stream))
                .unwrap()
        }
    }
}

fn tool(id: &str, name: &str, input: Value) -> Response {
    sse(json!({"choices": [{
        "index": 0,
        "delta": {"tool_calls": [{
            "index": 0,
            "id": id,
            "type": "function",
            "function": {"name": name, "arguments": input.to_string()}
        }]},
        "finish_reason": "tool_calls"
    }]}))
}

fn text(content: &str) -> Response {
    sse(json!({"choices": [{
        "index": 0, "delta": {"content": content}, "finish_reason": "stop"
    }]}))
}

fn sse(value: Value) -> Response {
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from(format!("data: {value}\n\ndata: [DONE]\n\n")))
        .unwrap()
}
