//! `bash` — persistent-shell command execution with approval gating,
//! timeouts, an environment allowlist, and head+tail output truncation.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;

use super::{truncate_with_tempfile, Tool, ToolCtx, ToolError, ToolOutput};

/// Only these variables pass through to spawned shells (spec §7).
const ENV_ALLOWLIST: &[&str] = &[
    "PATH", "HOME", "SHELL", "TERM", "LANG", "LC_ALL", "TMPDIR", "USER", "LOGNAME",
];
const DEFAULT_TIMEOUT_SECS: u64 = 60;
const MAX_TIMEOUT_SECS: u64 = 600;
/// Marker byte (SOH) delimiting the embedded cwd probe on stderr.
const MARKER: char = '\u{01}';
const MARKER_TAG: &str = "HARNESS_CWD:";
const ABORT_POLL: Duration = Duration::from_millis(150);

#[derive(Debug)]
pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Run a shell command (`sh -c`). The working directory persists across \
         calls within the session. Output over ~16k chars is truncated \
         head+tail; the full text is written to a temp file whose path is \
         included. Requires approval unless the command matches an allowed \
         prefix."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command line(s) to execute."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Seconds before the process is killed (default 60, max 600)."
                }
            },
            "required": ["command"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input
            .as_object()
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "bash",
                problem: "input must be an object".into(),
            })?;
        let command = obj
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "bash",
                problem: "`command` must be a non-empty string".into(),
            })?;
        let timeout_secs = obj
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .clamp(1, MAX_TIMEOUT_SECS);

        if ctx.aborted() {
            return Err(ToolError::Failed("aborted".into()));
        }

        let start_cwd = ctx
            .shell_cwd
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| ctx.project_root.clone());

        let script = build_script(&start_cwd, command);
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c")
            .arg(&script)
            .current_dir(&ctx.project_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        // Own process group ⇒ timeouts/aborts can kill the whole tree
        // (grandchildren like `sleep` would otherwise inherit the pipes and
        // keep draining blocked until they exit naturally).
        #[cfg(unix)]
        cmd.process_group(0);
        for key in ENV_ALLOWLIST {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }

        let mut child = cmd.spawn().map_err(|e| ToolError::Failed(format!("spawn failed: {e}")))?;
        let stdout_handle = drain(child.stdout.take());
        let stderr_handle = drain(child.stderr.take());

        let timeout_dur = Duration::from_secs(timeout_secs);
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            tokio::select! {
                waited = child.wait() => break waited.map_err(|e| ToolError::Failed(format!("wait failed: {e}")))?,
                _ = tokio::time::sleep(ABORT_POLL) => {
                    if ctx.aborted() {
                        kill_tree(&mut child);
                        let _ = child.wait().await;
                        return Err(ToolError::Failed("aborted".into()));
                    }
                    if started.elapsed() >= timeout_dur {
                        timed_out = true;
                        kill_tree(&mut child);
                        break child
                            .wait()
                            .await
                            .map_err(|e| ToolError::Failed(format!("wait failed: {e}")))?;
                    }
                }
            }
        };

        let stdout = stdout_handle
            .await
            .unwrap_or_default();
        let mut stderr = stderr_handle.await.unwrap_or_default();

        // Harvest + strip the persistent-cwd marker from stderr.
        let new_cwd = extract_marker(&mut stderr);
        if let Some(dir) = new_cwd {
            if let Ok(mut guard) = ctx.shell_cwd.lock() {
                if dir.is_dir() {
                    tracing::debug!(from = %guard.display(), to = %dir.display(), "shell cwd changed");
                    *guard = dir;
                }
            }
        }

        let code = status.code().unwrap_or(-1);
        let mut body = String::new();
        if timed_out {
            body.push_str(&format!(
                "[killed after {timeout_secs}s timeout]\n"
            ));
        } else if ctx.aborted() {
            body.push_str("[aborted by user]\n");
        }
        body.push_str(&format!("exit code: {code}\n"));
        if !stdout.is_empty() {
            body.push_str("--- stdout ---\n");
            body.push_str(&stdout);
        }
        if !stderr.is_empty() {
            body.push_str("--- stderr ---\n");
            body.push_str(&stderr);
        }

        let result = truncate_with_tempfile(&body, ctx);
        let first_line = command.lines().next().unwrap_or(command);
        let summary = if timed_out {
            format!("bash (timed out): {first_line}")
        } else {
            format!("bash ({code}): {first_line}")
        };

        tracing::debug!(tool = "bash", exit = code, timed_out, "command finished");
        Ok(if status.success() && !timed_out {
            ToolOutput::success(result, summary)
        } else {
            ToolOutput::failure(result, summary)
        })
    }
}

/// Kill the child and everything it spawned: the child leads its own
/// process group (set at spawn), so a group SIGKILL reaches grandchildren.
fn kill_tree(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        // `kill -9 -PGID` — safe-Rust path via the system kill binary,
        // keeping the workspace-wide `unsafe_code = "forbid"` intact.
        let _ = std::process::Command::new("kill")
            .arg("-9")
            .arg(format!("-{pid}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.start_kill();
}

fn drain<R>(pipe: Option<R>) -> tokio::task::JoinHandle<String>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf).await;
        }
        String::from_utf8_lossy(&buf).into_owned()
    })
}

fn build_script(start_cwd: &Path, command: &str) -> String {
    format!(
        "cd {} || true\n{}\nstatus=$?\nprintf '\\{}{}%s\\{}' \"$PWD\" >&2\nexit $status\n",
        shell_quote(&start_cwd.to_string_lossy()),
        command,
        MARKER as u32,
        MARKER_TAG,
        MARKER as u32
    )
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Remove the trailing cwd marker from `stderr`, returning the recorded dir.
fn extract_marker(stderr: &mut String) -> Option<std::path::PathBuf> {
    let needle = format!("{MARKER}{MARKER_TAG}");
    let start = stderr.find(&needle)?;
    let rest = &stderr[start + needle.len()..];
    let end = rest.find(MARKER)?;
    let dir_text = rest[..end].to_string();
    // Cut everything from the marker onward, trimming the newline before it.
    *stderr = stderr[..start].trim_end_matches('\n').to_string();
    if dir_text.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(dir_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};

    fn ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    async fn run_cmd(ctx: &ToolCtx, json: Value) -> Result<ToolOutput, ToolError> {
        BashTool.run(json, ctx).await
    }

    #[tokio::test]
    async fn captures_stdout_exit_code_and_stderr() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        let out = run_cmd(
            &ctx,
            json!({"command": "echo hello; echo oops >&2; exit 3"}),
        )
        .await
        .unwrap();
        assert!(!out.ok);
        assert!(out.result.contains("exit code: 3"));
        assert!(out.result.contains("hello"));
        assert!(out.result.contains("oops"));
        assert!(!out.result.contains("HARNESS_CWD")); // marker never leaks
    }

    #[tokio::test]
    async fn cwd_persists_across_calls_and_is_reported_by_pwd() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        run_cmd(&ctx, json!({"command": "mkdir -p sub && cd sub"}))
            .await
            .unwrap();
        let out = run_cmd(&ctx, json!({"command": "pwd"})).await.unwrap();
        let expected = ctx.shell_cwd.lock().unwrap().clone();
        assert!(expected.ends_with("sub"));
        assert!(
            out.result.contains(expected.to_string_lossy().trim()),
            "pwd output {:?} should contain {:?}",
            out.result,
            expected
        );
    }

    #[tokio::test]
    async fn timeout_kills_the_process() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        let out = run_cmd(
            &ctx,
            json!({"command": "sleep 30", "timeout_secs": 1}),
        )
        .await
        .unwrap();
        assert!(!out.ok);
        assert!(out.result.contains("[killed after 1s timeout]"));
        assert!(out.summary.contains("timed out"));
    }

    #[tokio::test]
    async fn abort_flag_stops_a_running_command_quickly() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        let ctx2 = ctx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            ctx2.abort.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        let err = run_cmd(&ctx, json!({"command": "sleep 30"})).await;
        assert!(err.is_err()); // ToolError::Failed("aborted")
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn env_is_filtered_to_allowlist() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        // PATH is allowlisted and must survive; CARGO_* vars set for this
        // test process are not on the list and must not leak into children.
        let out = run_cmd(
            &ctx,
            json!({"command": "printenv PATH >/dev/null && echo has-path; printenv CARGO_MANIFEST_DIR >/dev/null 2>&1 && echo leaked || echo clean"}),
        )
        .await
        .unwrap();
        assert!(out.result.contains("has-path"));
        assert!(out.result.contains("clean"));
        assert!(!out.result.contains("leaked"));
    }

    #[tokio::test]
    async fn invalid_input_shapes_are_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        assert!(run_cmd(&ctx, json!([1, 2])).await.is_err());
        assert!(run_cmd(&ctx, json!({"command": "   "})).await.is_err());
    }

    #[test]
    fn marker_extraction_roundtrip() {
        let mut stderr = format!("some noise\n{MARKER}{MARKER_TAG}/tmp/x/y{MARKER}\n");
        let dir = extract_marker(&mut stderr);
        assert_eq!(dir, Some(std::path::PathBuf::from("/tmp/x/y")));
        assert_eq!(stderr, "some noise");

        let mut plain = "just errors".to_string();
        assert_eq!(extract_marker(&mut plain), None);
        assert_eq!(plain, "just errors");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}
