//! Minimal MCP (Model Context Protocol) stdio client (spec §9 v0.9).
//!
//! Speaks newline-delimited JSON-RPC 2.0 to a third-party server process:
//! `initialize` → `notifications/initialized` → `tools/list` → per-call
//! `tools/call`. One connection per configured server; crashed servers are
//! restarted lazily on next use.

pub mod tool_adapter;

use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

const INIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Default)]
struct Shared {
    pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>,
    tools: Mutex<Vec<McpToolInfo>>,
    dead: AtomicBool,
    ready: AtomicBool,
    attempts: AtomicU32,
}

/// A live connection to one MCP server.
#[derive(Clone)]
pub struct McpConnection {
    inner: Arc<McpInner>,
}

pub struct McpInner {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub project_root: PathBuf,
    pub shared: Arc<Shared>,
    pub conn: Mutex<Option<Conn>>,
    pub next_id: AtomicI64,
}

#[derive(Debug)]
struct Conn {
    child: Child,
    stdin: tokio::process::ChildStdin,
}

impl McpConnection {
    pub fn new(name: &str, command: &str, args: &[String], project_root: &Path) -> Self {
        Self {
            inner: Arc::new(McpInner {
                name: name.to_string(),
                command: command.to_string(),
                args: args.to_vec(),
                project_root: project_root.to_path_buf(),
                shared: Arc::new(Shared {
                    pending: Mutex::new(HashMap::new()),
                    tools: Mutex::new(Vec::new()),
                    dead: AtomicBool::new(false),
                    ready: AtomicBool::new(false),
                    attempts: AtomicU32::new(0),
                }),
                conn: Mutex::new(None),
                next_id: AtomicI64::new(1),
            }),
        }
    }

    fn spawn_args(&self) -> (&str, &[String], &Path) {
        (
            &self.inner.command,
            &self.inner.args,
            &self.inner.project_root,
        )
    }

    pub async fn ensure(&self) -> Result<(), String> {
        self.ensure_impl().await
    }

    async fn ensure_impl(&self) -> Result<(), String> {
        if self.inner.shared.ready.load(Ordering::Relaxed)
            && !self.inner.shared.dead.load(Ordering::Relaxed)
        {
            return Ok(());
        }
        if self.inner.shared.attempts.load(Ordering::Relaxed) >= 3 {
            return Err(format!("mcp server '{}' unreachable", self.inner.name));
        }
        self.inner.shared.attempts.fetch_add(1, Ordering::Relaxed);
        let (command, args, cwd) = self.spawn_args();

        let mut child = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn mcp '{}': {e}", self.inner.name))?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");

        *self.inner.conn.lock().await = Some(Conn { child, stdin });
        self.inner.shared.dead.store(false, Ordering::Relaxed);

        // Reader: newline-delimited JSON-RPC.
        let shared = Arc::clone(&self.inner.shared);
        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(&line) {
                    Ok(v) => {
                        if tx.send(v).is_err() {
                            break;
                        }
                    }
                    Err(_) => continue,
                }
            }
            drop(tx);
            shared.dead.store(true, Ordering::Relaxed);
            shared.ready.store(false, Ordering::Relaxed);
        });

        let dispatcher = Arc::clone(&self.inner.shared);
        tokio::spawn(async move {
            while let Some(v) = rx.recv().await {
                dispatch(&dispatcher, v).await;
            }
        });

        // Handshake.
        let result = self
            .call_locked(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "harness", "version": env!("CARGO_PKG_VERSION")}
                }),
                INIT_TIMEOUT,
            )
            .await?;
        let _server_info = result; // capabilities ignored in v0.9
        let init_note = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        self.write_line(&init_note.to_string()).await?;

        // Cache tool list (low-level call: ensure already done).
        let tools_result = self
            .call_locked("tools/list", json!({}), INIT_TIMEOUT)
            .await?;
        let mut infos = Vec::new();
        if let Some(list) = tools_result["tools"].as_array() {
            for t in list {
                infos.push(McpToolInfo {
                    name: t["name"].as_str().unwrap_or_default().to_string(),
                    description: t["description"].as_str().unwrap_or_default().to_string(),
                    input_schema: t["inputSchema"].clone(),
                });
            }
        }
        *self.inner.shared.tools.lock().await = infos;
        self.inner.shared.ready.store(true, Ordering::Relaxed);
        self.inner.shared.attempts.store(0, Ordering::Relaxed);
        tracing::info!(server = %self.inner.name, "mcp server initialized");
        Ok(())
    }

    /// Cached tool inventory (after a successful handshake).
    pub async fn list_tools(&self) -> Vec<McpToolInfo> {
        self.inner.shared.tools.lock().await.clone()
    }

    pub async fn call_tool(&self, tool: &str, arguments: Value) -> Result<String, String> {
        let result = self
            .call_inner(
                "tools/call",
                json!({"name": tool, "arguments": arguments}),
                CALL_TIMEOUT,
            )
            .await?;
        // Result shape: { content: [ {type:"text", text:"..."}, ... ], isError? }
        if result["isError"].as_bool() == Some(true) {
            return Err(extract_text(&result).unwrap_or_else(|| "tool error".into()));
        }
        Ok(extract_text(&result).unwrap_or_default())
    }

    async fn call_inner(
        &self,
        method: &str,
        params: Value,
        timeout: std::time::Duration,
    ) -> Result<Value, String> {
        self.ensure().await?;
        self.call_locked(method, params, timeout).await
    }

    async fn call_locked(
        &self,
        method: &str,
        params: Value,
        timeout: std::time::Duration,
    ) -> Result<Value, String> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});

        let (tx, rxx) = oneshot::channel();
        self.inner.shared.pending.lock().await.insert(id, tx);
        if let Err(e) = self.write_line(&body.to_string()).await {
            self.inner.shared.pending.lock().await.remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(timeout, rxx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err("mcp response channel dropped".into()),
            Err(_) => {
                self.inner.shared.pending.lock().await.remove(&id);
                Err(format!("{method} timed out"))
            }
        }
    }

    async fn write_line(&self, body: &str) -> Result<(), String> {
        let mut guard = self.inner.conn.lock().await;
        let Some(conn) = guard.as_mut() else {
            return Err("no mcp connection".into());
        };
        conn.stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|e| format!("mcp write: {e}"))?;
        conn.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("mcp write nl: {e}"))?;
        conn.stdin
            .flush()
            .await
            .map_err(|e| format!("mcp flush: {e}"))
    }

    async fn notify(&self, method: &str) -> Result<(), String> {
        self.ensure().await?;
        let body = json!({"jsonrpc":"2.0","method":method});
        self.write_line(&body.to_string()).await
    }
}

fn extract_text(result: &Value) -> Option<String> {
    let arr = result["content"].as_array()?;
    let mut out = String::new();
    for item in arr {
        if item["type"].as_str() == Some("text") {
            out.push_str(item["text"].as_str().unwrap_or_default());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

async fn dispatch(shared: &Arc<Shared>, v: Value) {
    if let Some(id) = v.get("id").and_then(Value::as_i64) {
        let result = if v.get("error").is_some() {
            Err(v["error"]["message"]
                .as_str()
                .unwrap_or("error")
                .to_string())
        } else {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        };
        if let Some(tx) = shared.pending.lock().await.remove(&id) {
            let _ = tx.send(result);
        }
    }
}
