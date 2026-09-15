use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use serde_json::{Value, json};
use z_engine_core::agent::{AgentHandle, Event, EventRx, LoopConfig};

#[derive(Clone)]
struct Script {
    responses: Arc<Mutex<VecDeque<Option<Value>>>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

pub struct Provider {
    pub base_url: String,
    script: Script,
    task: tokio::task::JoinHandle<()>,
}

impl Provider {
    /// A missing response holds the stream open for the crash fixture.
    pub async fn start(responses: Vec<Option<Value>>) -> Self {
        let script = Script {
            responses: Arc::new(Mutex::new(responses.into())),
            requests: Arc::default(),
        };
        let app = axum::Router::new()
            .route("/chat/completions", post(handler))
            .with_state(script.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url: format!("http://{address}"),
            script,
            task,
        }
    }

    pub fn requests(&self) -> Vec<Value> {
        self.script.requests.lock().unwrap().clone()
    }

    pub fn assert_consumed(&self) {
        assert!(self.script.responses.lock().unwrap().is_empty());
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
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
    let response = if is_title {
        answer("Desktop acceptance fixture")
    } else {
        script.requests.lock().unwrap().push(request);
        script
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected model request after the scripted scenario")
    };
    let body = match response {
        Some(value) => Body::from(format!("data: {value}\n\ndata: [DONE]\n\n")),
        None => Body::from_stream(futures::stream::pending::<
            Result<axum::body::Bytes, std::io::Error>,
        >()),
    };
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap()
}

pub fn tool(id: &str, name: &str, input: Value) -> Option<Value> {
    Some(json!({"choices": [{
        "index": 0,
        "delta": {"tool_calls": [{
            "index": 0, "id": id, "type": "function",
            "function": {"name": name, "arguments": input.to_string()}
        }]},
        "finish_reason": "tool_calls"
    }]}))
}

pub fn answer(content: &str) -> Option<Value> {
    Some(json!({"choices": [{
        "index": 0, "delta": {"content": content}, "finish_reason": "stop"
    }]}))
}

pub fn config(provider: &Provider, workspace: &Path) -> LoopConfig {
    let mut config = LoopConfig::new("desktop-acceptance-model", &provider.base_url);
    config.api_key = Some("local-fixture-not-a-secret".into());
    config.project_root = workspace.to_path_buf();
    config.tmp_dir = workspace.join("tool-output");
    config.review_enabled = false;
    config.max_task_continuations = 0;
    config
}

pub async fn next_event(events: &mut EventRx) -> Event {
    let event = tokio::time::timeout(Duration::from_secs(60), events.recv())
        .await
        .expect("agent event timed out")
        .expect("agent stopped before completing the scenario");
    assert!(
        !matches!(event, Event::Error(_) | Event::TurnAborted),
        "unexpected agent event: {event:?}"
    );
    event
}

pub async fn shutdown(handle: AgentHandle, mut events: EventRx) {
    handle.shutdown();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = events.recv().await {
            assert!(!matches!(event, Event::Error(_)), "{event:?}");
        }
    })
    .await
    .expect("agent did not shut down");
}
