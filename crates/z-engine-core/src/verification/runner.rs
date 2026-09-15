use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::artifacts::{MAX_ARTIFACT_BYTES, digest_file};
use super::process::{CHECK_TIMEOUT, execute};
use super::{CheckEvidence, CheckKind, CheckOutcome, CheckSpec, EvidenceArtifact};
use super::{VerificationError, WorkspaceSnapshot};

/// Run one fixed-argv Cargo check and retain durable, checksummed process output.
/// Unsupported scope/process failures are evidence; artifact persistence errors are errors.
pub async fn run_check(
    root: &Path,
    spec: CheckSpec,
    evidence_dir: &Path,
    abort: Arc<AtomicBool>,
) -> Result<CheckEvidence, VerificationError> {
    let started = Instant::now();
    let mut evidence = super::blocked_evidence(root, spec, "")?;
    let root = root.to_path_buf();
    let directory = evidence_dir.to_path_buf();
    let artifact_id = evidence.id.clone();
    let outputs =
        tokio::task::spawn_blocking(move || create_outputs(&directory, &artifact_id)).await??;
    if abort.load(Ordering::Relaxed) {
        evidence.outcome = CheckOutcome::Cancelled;
        evidence.summary = "cancelled before verification".into();
    } else {
        let capture_root = root.clone();
        let before = tokio::task::spawn_blocking(move || {
            let snapshot = WorkspaceSnapshot::capture(&capture_root)?;
            let identity = super::toolchain::identity(&snapshot.root)?;
            Ok::<_, VerificationError>((snapshot, identity))
        })
        .await?;
        match before {
            Err(error) => evidence.summary = error.to_string(),
            Ok((before, toolchain)) => {
                evidence.cwd = before.root.to_string_lossy().into_owned();
                evidence.input_fingerprint = Some(before.fingerprint.clone());
                evidence.toolchain = toolchain;
                let result = execute(
                    &before.root,
                    &evidence.command,
                    [&outputs[0], &outputs[1]],
                    abort,
                    CHECK_TIMEOUT,
                )
                .await?;
                evidence.exit_code = result.code;
                evidence.outcome = result.outcome;
                evidence.summary = result.message;
                let after_root = before.root.clone();
                let after = tokio::task::spawn_blocking(move || {
                    let snapshot = WorkspaceSnapshot::capture(&after_root)?;
                    let identity = super::toolchain::identity(&snapshot.root)?;
                    Ok::<_, VerificationError>((snapshot, identity))
                })
                .await?;
                match after {
                    Ok((after, identity))
                        if before.fingerprint == after.fingerprint
                            && evidence.toolchain == identity => {}
                    Ok(_) => {
                        evidence.outcome = CheckOutcome::Stale;
                        evidence.summary =
                            "workspace or toolchain changed during verification".into();
                    }
                    Err(error) => {
                        evidence.outcome = CheckOutcome::Stale;
                        evidence.summary = format!("cannot verify post-check inputs: {error}");
                    }
                }
            }
        }
    }
    let artifact_paths = outputs.clone();
    let (artifacts, counts) = tokio::task::spawn_blocking(move || {
        let artifacts = artifact_paths
            .iter()
            .map(|path| {
                Ok(EvidenceArtifact {
                    path: path.to_string_lossy().into_owned(),
                    digest: digest_file(path, MAX_ARTIFACT_BYTES)?.0,
                })
            })
            .collect::<Result<Vec<_>, VerificationError>>()?;
        let counts = test_counts(&artifact_paths[0])?;
        Ok::<_, VerificationError>((artifacts, counts))
    })
    .await??;
    evidence.stdout = Some(artifacts[0].clone());
    evidence.stderr = Some(artifacts[1].clone());
    if evidence.spec.kind == CheckKind::CargoTest {
        evidence.tests_run = counts.map(|(executed, _)| executed);
        if evidence.outcome == CheckOutcome::Passed {
            match counts {
                Some((tests, 0)) if tests > 0 => {
                    evidence.summary = format!("Cargo test passed: {tests} tests executed");
                }
                Some((_, failed)) if failed > 0 => {
                    evidence.outcome = CheckOutcome::Failed;
                    evidence.summary =
                        "test summaries report failed tests despite process success".into();
                }
                _ => {
                    evidence.outcome = CheckOutcome::Blocked;
                    evidence.summary =
                        "no positive standard Rust test count; unsupported or zero-test harness"
                            .into();
                }
            }
        }
    }
    evidence.duration_ms = started.elapsed().as_millis() as u64;
    evidence.summary = evidence.summary.chars().take(2000).collect();
    Ok(evidence)
}

fn create_outputs(directory: &Path, id: &str) -> Result<[PathBuf; 2], VerificationError> {
    std::fs::create_dir_all(directory).map_err(|e| VerificationError::io(directory, e))?;
    let directory = directory
        .canonicalize()
        .map_err(|e| VerificationError::io(directory, e))?;
    let outputs = [
        directory.join(format!("{id}.stdout.log")),
        directory.join(format!("{id}.stderr.log")),
    ];
    for path in &outputs {
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|e| VerificationError::io(path, e))?;
        file.sync_all()
            .map_err(|e| VerificationError::io(path, e))?;
    }
    #[cfg(unix)]
    std::fs::File::open(&directory)
        .and_then(|file| file.sync_all())
        .map_err(|e| VerificationError::io(&directory, e))?;
    Ok(outputs)
}

pub(super) fn test_counts(path: &Path) -> Result<Option<(u64, u64)>, VerificationError> {
    let file = std::fs::File::open(path).map_err(|e| VerificationError::io(path, e))?;
    let mut totals = None;
    for line in std::io::BufReader::new(file).lines() {
        let line = line.map_err(|e| VerificationError::io(path, e))?;
        if let Some((tests, failed)) = parse_summary(&line) {
            let (total, failures) = totals.get_or_insert((0_u64, 0_u64));
            *total = total
                .checked_add(tests)
                .ok_or_else(|| VerificationError::InvalidInput("test count overflow".into()))?;
            *failures = failures.checked_add(failed).ok_or_else(|| {
                VerificationError::InvalidInput("failed test count overflow".into())
            })?;
        }
    }
    Ok(totals)
}

pub(super) fn parse_summary(line: &str) -> Option<(u64, u64)> {
    let summary = line
        .strip_prefix("test result: ")?
        .strip_prefix("ok. ")
        .or_else(|| line.strip_prefix("test result: FAILED. "))?;
    let mut segments = summary.split("; ");
    let passed: u64 = segments.next()?.strip_suffix(" passed")?.parse().ok()?;
    let failed: u64 = segments.next()?.strip_suffix(" failed")?.parse().ok()?;
    let _: u64 = segments.next()?.strip_suffix(" ignored")?.parse().ok()?;
    Some((passed.checked_add(failed)?, failed))
}
