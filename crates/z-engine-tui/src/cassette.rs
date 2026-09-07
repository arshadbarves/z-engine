//! Cassette wiring: how a headless run is taped, or served from a tape.
//!
//! Two constructors, deliberately not one. [`recording`] needs a live
//! provider and therefore a key; [`replaying`] takes neither — there is no
//! parameter to pass a credential through, so a replayed run cannot reach
//! the network even by mistake. Both hand back the recorder the agent
//! loop needs, because a replayed guarded run still writes its own tape:
//! it re-asks its own gates and re-runs its own verification, and the
//! recorded evidence ids are what let its requests match.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use z_engine_core::replay::{RecordingProvider, ReplayProvider, RunCassette, RunRecorder};
use z_engine_core::z_engine_provider::{ChatProvider, Client};

/// A provider that tapes what it says, plus the recorder it writes to.
pub struct Taped {
    pub provider: Arc<dyn ChatProvider>,
    pub recorder: Arc<RunRecorder>,
    /// Where the run's own tape landed — the file metrics are read from.
    pub tape: PathBuf,
}

/// Record a live run onto `tape`.
pub fn recording(tape: &Path, base_url: &str, api_key: Option<String>) -> Result<Taped> {
    let recorder = RunRecorder::recording(tape)
        .with_context(|| format!("opening cassette {} for recording", tape.display()))?;
    let live = Client::new(base_url, api_key).context("building the provider client")?;
    Ok(Taped {
        provider: Arc::new(RecordingProvider::new(
            Arc::new(live) as Arc<dyn ChatProvider>,
            Arc::clone(&recorder),
        )),
        recorder,
        tape: tape.to_path_buf(),
    })
}

/// Serve a run from `tape`, taping the replay beside it.
pub fn replaying(tape: &Path) -> Result<Taped> {
    let cassette =
        RunCassette::load(tape).with_context(|| format!("loading cassette {}", tape.display()))?;
    let provider = ReplayProvider::new(&cassette)
        .with_context(|| format!("replaying cassette {}", tape.display()))?;
    let destination = replay_tape_path(tape);
    let recorder = RunRecorder::replaying(&destination, &cassette).with_context(|| {
        format!(
            "opening cassette {} for the replayed run",
            destination.display()
        )
    })?;
    Ok(Taped {
        provider: Arc::new(RecordingProvider::new(
            Arc::new(provider) as Arc<dyn ChatProvider>,
            Arc::clone(&recorder),
        )),
        recorder,
        tape: destination,
    })
}

/// Where a replay writes its own tape: beside the one it reads, under a
/// name of its own.
///
/// One cassette records exactly one run, so the destination cannot be
/// derived from the source alone — a tape worth keeping is one that can
/// be replayed again tomorrow, and a fixed name would refuse the second
/// attempt.
fn replay_tape_path(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "run".to_string());
    let extension = source
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let id = ulid::Ulid::new();
    source.with_file_name(format!("{stem}.replay-{id}{extension}"))
}

/// Copy the run's metrics out of its tape as JSON.
pub fn write_metrics(tape: &Path, destination: &Path) -> Result<()> {
    let cassette = RunCassette::load(tape)
        .with_context(|| format!("reading metrics from {}", tape.display()))?;
    let metrics = cassette
        .metrics()
        .context("the run recorded no metrics: it did not reach the end of a turn")?;
    let body = serde_json::to_string_pretty(&metrics)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(destination, format!("{body}\n"))
        .with_context(|| format!("writing metrics to {}", destination.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replay_writes_beside_the_tape_it_reads_under_a_name_of_its_own() {
        let first = replay_tape_path(Path::new("/tapes/run.jsonl"));
        assert_eq!(first.parent(), Some(Path::new("/tapes")));
        let named = first.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            named.starts_with("run.replay-") && named.ends_with(".jsonl"),
            "{named}"
        );
        assert_ne!(
            first,
            replay_tape_path(Path::new("/tapes/run.jsonl")),
            "the same cassette must be replayable twice"
        );
        assert!(
            replay_tape_path(Path::new("/tapes/run"))
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("run.replay-")
        );
    }

    #[test]
    fn replaying_a_missing_cassette_says_which_one() {
        let vault = tempfile::tempdir().unwrap();
        let missing = vault.path().join("nope.jsonl");
        let err = match replaying(&missing) {
            Ok(_) => panic!("a cassette that does not exist cannot be replayed"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("nope.jsonl"), "{err}");
    }

    #[test]
    fn metrics_from_a_run_that_never_ended_are_refused() {
        let vault = tempfile::tempdir().unwrap();
        let tape = vault.path().join("run.jsonl");
        let _recorder = RunRecorder::recording(&tape).unwrap();
        let err = write_metrics(&tape, &vault.path().join("m.json"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("recorded no metrics"), "{err}");
    }
}
