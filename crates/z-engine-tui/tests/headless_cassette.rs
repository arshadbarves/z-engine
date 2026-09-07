//! The headless binary, as an operator invokes it.
//!
//! Everything here runs the real `zengine` executable: the point is that
//! the flags reach the agent loop, that a run leaves a cassette and its
//! metrics on disk, and — the claim worth locking — that replaying that
//! cassette needs no server and no credential at all.

use std::path::{Path, PathBuf};
use std::process::Command;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One canned SSE answer, served to every request.
const REPLY: &str = concat!(
    "data: {\"choices\":[{\"delta\":{\"content\":\"done.\"}}]}\n\n",
    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],",
    "\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\n",
    "data: [DONE]\n\n"
);

/// A provider that answers anything, so the run has something to record.
async fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut scratch = [0u8; 8192];
                // One request per connection is all the client sends.
                let _ = socket.read(&mut scratch).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    REPLY.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(REPLY.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    format!("http://{addr}/v1")
}

/// Run the binary with a home of its own, so a test never reads the
/// developer's config, key, or sessions — nor writes to them.
fn zengine(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_zengine"))
        .args(args)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("ZENGINE_CONFIG", home.join("config/z-engine/config.toml"))
        .env("ZENGINE_API_KEY", "test-key-not-real")
        .output()
        .expect("the zengine binary must run")
}

fn metrics(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

struct Sandbox {
    home: tempfile::TempDir,
    project: tempfile::TempDir,
    vault: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            project: tempfile::tempdir().unwrap(),
            vault: tempfile::tempdir().unwrap(),
        }
    }

    fn tape(&self) -> PathBuf {
        self.vault.path().join("run.jsonl")
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recorded_headless_run_leaves_a_cassette_and_its_metrics() {
    let sandbox = Sandbox::new();
    let base = serve().await;
    let tape = sandbox.tape();
    let metrics_out = sandbox.vault.path().join("metrics.json");

    let run = zengine(
        sandbox.home.path(),
        &[
            "--project",
            sandbox.project.path().to_str().unwrap(),
            "--base-url",
            &base,
            "--headless",
            "say done",
            "--record-run",
            tape.to_str().unwrap(),
            "--metrics-out",
            metrics_out.to_str().unwrap(),
        ],
    );

    assert!(
        run.status.success(),
        "the run must succeed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(tape.is_file(), "the run must have left its cassette");
    let recorded = metrics(&metrics_out);
    assert_eq!(recorded["outcome"], serde_json::json!("completed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn replaying_that_cassette_needs_neither_a_server_nor_a_key() {
    let sandbox = Sandbox::new();
    let base = serve().await;
    let tape = sandbox.tape();
    let project = sandbox.project.path().to_str().unwrap().to_string();

    let recorded = zengine(
        sandbox.home.path(),
        &[
            "--project",
            &project,
            "--base-url",
            &base,
            "--headless",
            "say done",
            "--record-run",
            tape.to_str().unwrap(),
        ],
    );
    assert!(
        recorded.status.success(),
        "recording must succeed first: {}",
        String::from_utf8_lossy(&recorded.stderr)
    );

    // Nothing to answer the run but the tape: no key in the environment,
    // and the recorded host is not listening for it either.
    let metrics_out = sandbox.vault.path().join("replayed.json");
    let replayed = Command::new(env!("CARGO_BIN_EXE_zengine"))
        .args([
            "--project",
            &project,
            "--headless",
            "say done",
            "--replay-run",
            tape.to_str().unwrap(),
            "--metrics-out",
            metrics_out.to_str().unwrap(),
        ])
        .env("HOME", sandbox.home.path())
        .env("XDG_DATA_HOME", sandbox.home.path().join("data"))
        .env("XDG_CONFIG_HOME", sandbox.home.path().join("config"))
        .env(
            "ZENGINE_CONFIG",
            sandbox.home.path().join("config/z-engine/config.toml"),
        )
        .env_remove("ZENGINE_API_KEY")
        .output()
        .expect("the zengine binary must run");

    assert!(
        replayed.status.success(),
        "the replay must reach the same end: {}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    assert_eq!(
        metrics(&metrics_out)["outcome"],
        serde_json::json!("completed")
    );
    assert!(
        sandbox.vault.path().join("run.replay.jsonl").is_file(),
        "a replayed run tapes itself beside the cassette it read"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_guarded_flag_reaches_the_loop() {
    let sandbox = Sandbox::new();
    let base = serve().await;

    let run = zengine(
        sandbox.home.path(),
        &[
            "--project",
            sandbox.project.path().to_str().unwrap(),
            "--base-url",
            &base,
            "--guarded",
            "--headless",
            "say done",
        ],
    );

    let said = String::from_utf8_lossy(&run.stderr);
    assert!(
        said.contains("guarded mode"),
        "a guarded run must announce its terms: {said}"
    );
}

#[test]
fn taping_an_interactive_session_is_refused_before_anything_starts() {
    let sandbox = Sandbox::new();
    let refused = zengine(
        sandbox.home.path(),
        &["--record-run", sandbox.tape().to_str().unwrap()],
    );
    assert_eq!(refused.status.code(), Some(2));
    let told = String::from_utf8_lossy(&refused.stderr);
    assert!(told.contains("--headless"), "{told}");
}

#[test]
fn a_cassette_inside_the_project_is_refused_before_anything_starts() {
    let sandbox = Sandbox::new();
    let inside = sandbox.project.path().join("run.jsonl");
    let refused = zengine(
        sandbox.home.path(),
        &[
            "--project",
            sandbox.project.path().to_str().unwrap(),
            "--headless",
            "say done",
            "--record-run",
            inside.to_str().unwrap(),
        ],
    );
    assert_eq!(refused.status.code(), Some(2));
    let told = String::from_utf8_lossy(&refused.stderr);
    assert!(told.contains("outside the project"), "{told}");
}
