//! Child-process plumbing for the `bash` tool: pipe draining (plain and
//! line-streaming) and whole-process-group termination.

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

/// Kill the child and everything it spawned: the child leads its own
/// process group (set at spawn), so a group SIGKILL reaches grandchildren.
pub(super) fn kill_tree(child: &mut tokio::process::Child) {
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

pub(super) fn drain<R>(pipe: Option<R>) -> tokio::task::JoinHandle<String>
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

/// Read a pipe line-by-line, call `on_line` for each complete line, and
/// return the full accumulated text.
pub(super) fn drain_with_callback<R>(
    pipe: Option<R>,
    mut on_line: impl FnMut(String) + Send + 'static,
) -> tokio::task::JoinHandle<String>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut all = Vec::new();
        if let Some(p) = pipe {
            let mut reader = tokio::io::BufReader::new(p);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        all.extend_from_slice(line.as_bytes());
                        on_line(line.trim_end_matches('\n').to_string());
                    }
                    Err(_) => break,
                }
            }
        }
        String::from_utf8_lossy(&all).into_owned()
    })
}
