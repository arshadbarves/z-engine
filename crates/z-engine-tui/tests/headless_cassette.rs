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

/// Read one HTTP request off `socket` to completion (headers, then the body
/// `Content-Length` promises) before this connection's reply is written.
///
/// A guarded run's system prompt is long enough to push the request past a
/// single `read()` call's worth of bytes. Responding — and dropping the
/// socket — while the kernel's receive buffer still holds unread request
/// bytes has the client's write racing the server's close: BSD-derived
/// stacks (macOS included) answer unread-data-at-close with `RST` rather
/// than a clean `FIN`, which the client sees as its response stream being
/// interrupted, not as anything about the request it sent. Draining the
/// exact byte count the client declared avoids the race for any request
/// size, guarded or not.
async fn drain_request(socket: &mut tokio::net::TcpStream) {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let content_length: usize = std::str::from_utf8(&buf[..header_end])
        .ok()
        .and_then(|headers| {
            headers.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().ok())
                    .flatten()
            })
        })
        .unwrap_or(0);
    let wanted = header_end + content_length;
    while buf.len() < wanted {
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

/// Position just past the blank line ending an HTTP header block, if the
/// buffer holds one yet.
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

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
                // One request per connection is all the client sends, but
                // it must be read in full before the reply — see
                // `drain_request`.
                drain_request(&mut socket).await;
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
    // and the recorded host is not listening for it either. Twice, because
    // a tape worth keeping is one that can be replayed again.
    for attempt in 1..=2 {
        let metrics_out = sandbox
            .vault
            .path()
            .join(format!("replayed-{attempt}.json"));
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
            "replay {attempt} must reach the same end: {}",
            String::from_utf8_lossy(&replayed.stderr)
        );
        assert_eq!(
            metrics(&metrics_out)["outcome"],
            serde_json::json!("completed")
        );
    }

    assert_eq!(
        replay_tapes(sandbox.vault.path()),
        2,
        "each replay tapes itself beside the cassette it read"
    );
}

/// How many tapes a replay has left in `vault`.
fn replay_tapes(vault: &Path) -> usize {
    std::fs::read_dir(vault)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("run.replay-"))
        .count()
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
        run.status.success(),
        "a guarded run over a working provider must succeed, not merely \
         mention guarded mode: status={:?} stderr={said}",
        run.status
    );
    const NOTE: &str =
        "guarded mode: reads are recorded as evidence; declare a work order before editing";
    assert!(
        said.contains(NOTE),
        "a guarded run must announce its terms with the exact success note {NOTE:?}: {said}"
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
