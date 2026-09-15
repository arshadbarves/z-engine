use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::artifacts::MAX_ARTIFACT_BYTES;
use super::{CheckOutcome, VerificationError};

pub(super) const CHECK_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug)]
pub(super) struct ProcessResult {
    pub code: Option<i32>,
    pub outcome: CheckOutcome,
    pub message: String,
}

struct OwnedChild {
    child: tokio::process::Child,
    pid: Option<u32>,
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            kill_group(pid);
            if let Err(error) = self.child.start_kill() {
                tracing::warn!(%error, "verification process drop cleanup failed");
            }
        }
    }
}

pub(super) fn kill_group(pid: u32) {
    #[cfg(unix)]
    let result = std::process::Command::new("kill")
        .env_clear()
        .envs(super::environment::effective())
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let result = std::process::Command::new("taskkill")
        .env_clear()
        .envs(super::environment::effective())
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(any(unix, windows))]
    match result {
        Err(error) => tracing::warn!(pid, %error, "verification process-group cleanup failed"),
        Ok(status) if !status.success() => {
            tracing::debug!(pid, %status, "process group already absent or could not be terminated");
        }
        Ok(_) => {}
    }
}

pub(super) async fn execute(
    root: &Path,
    command: &[String],
    outputs: [&Path; 2],
    abort: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<ProcessResult, VerificationError> {
    if abort.load(Ordering::Relaxed) {
        return Ok(ProcessResult {
            code: None,
            outcome: CheckOutcome::Cancelled,
            message: "cancelled before process spawn".into(),
        });
    }
    let files = outputs
        .iter()
        .map(|path| {
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|e| VerificationError::io(*path, e))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut command_builder = tokio::process::Command::new(&command[0]);
    super::environment::apply(command_builder.as_std_mut());
    command_builder
        .args(&command[1..])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(
            files[0]
                .try_clone()
                .map_err(|e| VerificationError::io(outputs[0], e))?,
        )
        .stderr(
            files[1]
                .try_clone()
                .map_err(|e| VerificationError::io(outputs[1], e))?,
        )
        .kill_on_drop(true);
    #[cfg(unix)]
    command_builder.process_group(0);
    #[cfg(windows)]
    command_builder.creation_flags(0x08000000);
    let child = match command_builder.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ok(ProcessResult {
                code: None,
                outcome: CheckOutcome::Blocked,
                message: format!("process spawn failed: {error}"),
            });
        }
    };
    let mut owned = OwnedChild {
        pid: child.id(),
        child,
    };
    let started = Instant::now();
    let mut forced = None;
    let status = loop {
        tokio::select! {
            status = owned.child.wait() => break status.map_err(|e| {
                VerificationError::Process(format!("process wait failed: {e}"))
            })?,
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                let oversized = files.iter().try_fold(false, |found, file| {
                    file.metadata().map(|metadata| found || metadata.len() > MAX_ARTIFACT_BYTES)
                }).map_err(|e| VerificationError::io(PathBuf::from(root), e))?;
                forced = if abort.load(Ordering::Relaxed) {
                    Some((CheckOutcome::Cancelled, "verification cancelled"))
                } else if started.elapsed() >= timeout {
                    Some((CheckOutcome::Blocked, "verification timed out"))
                } else if oversized {
                    Some((CheckOutcome::Blocked, "verification output exceeded artifact limit"))
                } else { None };
                if forced.is_some() {
                    if let Some(pid) = owned.pid { kill_group(pid); }
                    owned.child.start_kill().map_err(|e| VerificationError::Process(e.to_string()))?;
                    break owned.child.wait().await.map_err(|e| VerificationError::Process(e.to_string()))?;
                }
            }
        }
    };
    // Also terminate lingering descendants after the Cargo leader exits.
    if let Some(pid) = owned.pid.take() {
        kill_group(pid);
    }
    for (file, path) in files.iter().zip(outputs) {
        file.sync_all()
            .map_err(|e| VerificationError::io(path, e))?;
    }
    let (outcome, message) = forced
        .map(|(outcome, message)| (outcome, message.into()))
        .unwrap_or_else(|| {
            if status.success() {
                (CheckOutcome::Passed, "process exited successfully".into())
            } else {
                (CheckOutcome::Failed, format!("process exited {status}"))
            }
        });
    Ok(ProcessResult {
        code: status.code(),
        outcome,
        message,
    })
}
