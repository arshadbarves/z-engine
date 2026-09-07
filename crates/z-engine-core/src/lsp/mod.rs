//! Minimal LSP client for rust-analyzer (spec §9 v0.8).
//!
//! JSON-RPC over the child's stdio with Content-Length framing. Requests are
//! correlated by id through oneshot channels; `publishDiagnostics`
//! notifications are captured into a store that tools and the edit hook
//! read. A crashed server is transparently re-spawned on next use (bounded).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::{Duration, Instant};

const INIT_TIMEOUT: Duration = Duration::from_secs(20);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_SPAWN_ATTEMPTS: u32 = 3;
/// How long publishDiagnostics polling waits before giving up.
pub const DIAGNOSTICS_WAIT: Duration = Duration::from_millis(2500);

#[derive(Debug)]
struct Shared {
    diagnostics: Mutex<HashMap<String, Vec<Value>>>,
    /// Responses waiting for their id.
    pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>,
    server_gone: AtomicBool,
    ready: AtomicBool,
    spawn_attempts: AtomicU32,
    /// Bumped on every successful spawn. A document opened against an
    /// earlier generation means nothing to the server running now.
    generation: AtomicU64,
}

#[derive(Debug)]
pub struct LspClient {
    shared: Arc<Shared>,
    conn: Mutex<Option<Connection>>,
    /// Held across the whole of [`LspClient::open_document`], which is
    /// what serializes two concurrent opens of one document.
    open_docs: Mutex<open_docs::OpenDocs>,
    project_root: PathBuf,
    server: PathBuf,
}

#[derive(Debug)]
struct Connection {
    /// Held so the server dies with the client (kill_on_drop at spawn).
    _child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    next_id: i64,
}

impl Connection {
    fn next_id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }
}

pub mod batch;
pub mod cargo_check;
pub mod health;
mod open_docs;
pub mod symbols;
pub mod wire;

pub use health::LspHealth;
pub use symbols::{DeclaredSymbol, SymbolAnswer};
use wire::{
    did_change_params, did_open_params, find_frame, frame, is_response, percent_encode_path,
};

/// Public wrapper used by LSP tooling to build document URIs.
pub fn percent_encode_path_public(p: &Path) -> String {
    percent_encode_path(p)
}

impl LspClient {
    /// Probe whether an LSP server makes sense for this project: a
    /// Cargo.toml at the root plus a `rust-analyzer` binary on PATH.
    pub fn probe(project_root: &Path) -> Option<PathBuf> {
        if !project_root.join("Cargo.toml").exists() {
            return None;
        }
        let ok = std::process::Command::new("rust-analyzer")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            Some(PathBuf::from("rust-analyzer"))
        } else {
            None
        }
    }

    pub fn new(project_root: &Path, server: PathBuf) -> Self {
        Self {
            shared: Arc::new(Shared {
                diagnostics: Mutex::new(HashMap::new()),
                pending: Mutex::new(HashMap::new()),
                server_gone: AtomicBool::new(false),
                ready: AtomicBool::new(false),
                spawn_attempts: AtomicU32::new(0),
                generation: AtomicU64::new(0),
            }),
            conn: Mutex::new(None),
            open_docs: Mutex::new(open_docs::OpenDocs::default()),
            project_root: project_root.to_path_buf(),
            server,
        }
    }

    /// Ensure a healthy, initialized connection; respawn after crashes.
    ///
    /// Returns the generation of the connection the caller may use. A
    /// respawn returns a *new* generation, which is how document state
    /// built against the dead server is discarded rather than reused.
    async fn ensure(&self) -> Result<u64, String> {
        if self.shared.ready.load(Ordering::Relaxed)
            && !self.shared.server_gone.load(Ordering::Relaxed)
        {
            return Ok(self.shared.generation.load(Ordering::Relaxed));
        }
        if self.shared.spawn_attempts.load(Ordering::Relaxed) >= MAX_SPAWN_ATTEMPTS {
            return Err("lsp server unavailable (spawn attempts exhausted)".into());
        }
        self.shared.spawn_attempts.fetch_add(1, Ordering::Relaxed);

        let mut child = tokio::process::Command::new(&self.server)
            .current_dir(&self.project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn rust-analyzer: {e}"))?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");

        *self.conn.lock().await = Some(Connection {
            _child: child,
            stdin,
            next_id: 0,
        });
        self.shared.server_gone.store(false, Ordering::Relaxed);
        // A fresh server knows no documents, whatever the old one was
        // told. Bumping the generation is what says so; `open_docs`
        // rebuilds itself from it on the next synchronization.
        let generation = self.shared.generation.fetch_add(1, Ordering::Relaxed) + 1;

        // Reader: parse frames, route responses / diagnostics.
        let shared = Arc::clone(&self.shared);
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            let mut stdout = stdout;
            let mut buf = Vec::new();
            'outer: loop {
                let mut chunk = [0u8; 8192];
                match stdout.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                while let Some(frame_end) = find_frame(&buf) {
                    let body = buf[..frame_end].to_vec();
                    buf.drain(..frame_end);
                    if tx.send(body).is_err() {
                        break 'outer;
                    }
                }
            }
            drop(tx);
            shared.server_gone.store(true, Ordering::Relaxed);
            shared.ready.store(false, Ordering::Relaxed);
        });

        let shared2 = Arc::clone(&self.shared);
        tokio::spawn(async move {
            while let Some(body) = rx.recv().await {
                handle_message(Arc::clone(&shared2), body).await;
            }
        });

        // Initialize handshake.
        let root_uri = percent_encode_path(&self.project_root);
        let _ = self
            .raw_request(
                "initialize",
                json!({
                    "processId": std::process::id(),
                    "rootUri": root_uri,
                    "capabilities": {}
                }),
                INIT_TIMEOUT,
            )
            .await?;
        let body = json!({"jsonrpc":"2.0","method":"initialized","params":{}});
        self.write_frame_no_ensure(&body.to_string()).await?;
        self.shared.ready.store(true, Ordering::Relaxed);
        self.shared.spawn_attempts.store(0, Ordering::Relaxed);
        tracing::info!(generation, "lsp server initialized");
        Ok(generation)
    }

    async fn raw_request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let id = {
            let mut guard = self.conn.lock().await;
            let Some(conn) = guard.as_mut() else {
                return Err("no connection".into());
            };
            conn.next_id()
        };
        let body = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});

        let (tx, rxx) = oneshot::channel();
        self.shared.pending.lock().await.insert(id, tx);

        if let Err(e) = self.write_frame_no_ensure(&body.to_string()).await {
            self.shared.pending.lock().await.remove(&id);
            return Err(e);
        }

        match tokio::time::timeout(timeout, rxx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err("lsp response channel dropped".into()),
            Err(_) => {
                self.shared.pending.lock().await.remove(&id);
                Err(format!("{method} timed out"))
            }
        }
    }

    async fn write_frame_no_ensure(&self, body: &str) -> Result<(), String> {
        let _ = &self.server; // reserved for future per-server options

        let mut guard = self.conn.lock().await;
        let Some(conn) = guard.as_mut() else {
            return Err("no connection".into());
        };
        conn.stdin
            .write_all(&frame(body))
            .await
            .map_err(|e| format!("lsp write: {e}"))?;
        conn.stdin
            .flush()
            .await
            .map_err(|e| format!("lsp flush: {e}"))
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.ensure().await?;
        let body = json!({"jsonrpc":"2.0","method":method,"params":params});
        self.write_frame_no_ensure(&body.to_string()).await
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.ensure().await?;
        self.raw_request(method, params, REQUEST_TIMEOUT).await
    }

    /// Open (or fully replace) a document so the server analyzes current
    /// content. Uses `didChange` only when *this* connection opened it.
    ///
    /// The registry lock is held across the send, so two callers racing on
    /// the same file produce one `didOpen` and one `didChange` rather than
    /// two opens; and the notification is written directly rather than
    /// through [`LspClient::notify`], which would re-`ensure` and could
    /// respawn between the decision and the write.
    pub async fn open_document(&self, abs_path: &Path, text: &str) -> Result<(), String> {
        // Connect first: a notification written before the server exists is
        // lost, and the caller would then be told about stale on-disk bytes.
        let generation = self.ensure().await?;
        let uri = percent_encode_path(abs_path);
        let mut docs = self.open_docs.lock().await;
        let action = docs.action_for(&uri, generation);
        let params = match action {
            open_docs::OpenAction::DidOpen => did_open_params(&uri, text),
            open_docs::OpenAction::DidChange => did_change_params(&uri, next_version(), text),
        };
        let body = json!({"jsonrpc":"2.0","method":action.method(),"params":params});
        self.write_frame_no_ensure(&body.to_string()).await?;
        docs.confirm(&uri, generation);
        Ok(())
    }

    /// Snapshot of stored diagnostics for one uri.
    pub async fn diagnostics_for(&self, abs_path: &Path) -> Vec<Value> {
        let uri = percent_encode_path(abs_path);
        self.shared
            .diagnostics
            .lock()
            .await
            .get(&uri)
            .cloned()
            .unwrap_or_default()
    }

    /// Poll diagnostics for up to `wait`, returning the first non-empty set.
    pub async fn wait_diagnostics(&self, abs_path: &Path, wait: Duration) -> Vec<Value> {
        let deadline = Instant::now() + wait;
        loop {
            let d = self.diagnostics_for(abs_path).await;
            if !d.is_empty() || Instant::now() >= deadline {
                return d;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }
}

fn next_version() -> i64 {
    static V: AtomicI64 = AtomicI64::new(2);
    V.fetch_add(1, Ordering::SeqCst)
}

async fn handle_message(shared: Arc<Shared>, raw: Vec<u8>) {
    let body_start = match raw.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(i) => i + 4,
        None => return,
    };
    let Ok(v) = serde_json::from_slice::<Value>(&raw[body_start..]) else {
        return;
    };

    // Response to a request?
    if is_response(&v) {
        let id = v["id"].as_i64().unwrap_or_default();
        let result = if v.get("error").is_some() {
            Err(v["error"]["message"]
                .as_str()
                .unwrap_or("lsp error")
                .to_string())
        } else {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        };
        if let Some(tx) = shared.pending.lock().await.remove(&id) {
            let _ = tx.send(result);
        }
        return;
    }

    // Notification?
    if v.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
        let uri = v["params"]["uri"].as_str().unwrap_or_default().to_string();
        let diags = v["params"]["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if let Ok(mut map) = shared.diagnostics.try_lock() {
            map.insert(uri, diags);
        }
    }
}
