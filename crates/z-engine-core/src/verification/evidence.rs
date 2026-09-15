use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{CheckEvidence, CheckOutcome, CheckSpec, VerificationError};

/// Record a denied/unavailable check without inventing an execution or artifacts.
pub fn blocked_evidence(
    root: &Path,
    spec: CheckSpec,
    reason: impl Into<String>,
) -> Result<CheckEvidence, VerificationError> {
    let command = spec.command()?;
    Ok(CheckEvidence {
        id: ulid::Ulid::new().to_string(),
        spec,
        command,
        cwd: root.to_string_lossy().into_owned(),
        input_fingerprint: None,
        toolchain: String::new(),
        started_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| VerificationError::Process(error.to_string()))?
            .as_millis() as u64,
        duration_ms: 0,
        exit_code: None,
        tests_run: None,
        outcome: CheckOutcome::Blocked,
        summary: reason.into().chars().take(2000).collect(),
        stdout: None,
        stderr: None,
    })
}
