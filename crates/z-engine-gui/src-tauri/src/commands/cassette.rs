//! GUI run tapes: record a session's provider traffic onto a cassette, or
//! serve a fresh session from one. Mirrors `z-engine-tui/src/cassette.rs`.
//! A replayed run never uses an API key and never touches the network.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use z_engine_core::replay::{RecordingProvider, ReplayProvider, RunCassette, RunRecorder};
use z_engine_core::z_engine_provider::{ChatProvider, Client};

/// A provider that tapes what it says, plus the recorder it writes to.
pub(crate) struct Taped {
    pub(crate) provider: Arc<dyn ChatProvider>,
    pub(crate) recorder: Arc<RunRecorder>,
}

impl std::fmt::Debug for Taped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Taped").finish_non_exhaustive()
    }
}

pub(crate) fn pick_tape(record: Option<&str>, replay: Option<&str>) -> Result<(), String> {
    if record.is_some() && replay.is_some() {
        return Err("--record-run and --replay-run cannot be combined: a run is either served by a provider or by a cassette.".into());
    }
    Ok(())
}

pub(crate) fn check_resume_replay(
    resume: Option<&str>,
    replay: Option<&str>,
) -> Result<(), String> {
    if resume.is_some() && replay.is_some() {
        return Err("replay starts a fresh session: pass no resume_path with replay_run".into());
    }
    Ok(())
}

/// Resolve `tape` against the current dir and refuse it when it sits inside
/// the audited `root`: a guarded run accounts for every change under its
/// root, its own tape included.
pub(crate) fn check_tape_outside_project(tape: &Path, root: &Path) -> Result<PathBuf, String> {
    let absolute = if tape.is_absolute() {
        tape.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(tape)
    };
    let anchor = |p: &Path| {
        // The tape need not exist yet (it is created on open), and a bare
        // `canonicalize` fails on missing paths — losing symlink resolution
        // of the existing prefix (e.g. macOS `/var` -> `/private/var`).
        // Anchor the nearest existing ancestor and re-append the tail.
        let mut cur = p;
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        while !cur.exists() {
            match cur.parent() {
                Some(parent) => {
                    if let Some(name) = cur.file_name() {
                        tail.push(name.to_owned());
                    }
                    cur = parent;
                }
                None => break,
            }
        }
        let mut base = std::fs::canonicalize(cur).unwrap_or_else(|_| cur.to_path_buf());
        for comp in tail.iter().rev() {
            base.push(comp);
        }
        base
    };
    if anchor(&absolute).starts_with(anchor(root)) {
        return Err(format!(
            "tape must point outside the project: {} is inside {}",
            tape.display(),
            root.display()
        ));
    }
    Ok(absolute)
}

/// Record a live session onto `tape`.
pub(crate) fn recording(
    tape: &Path,
    base_url: &str,
    api_key: Option<String>,
) -> Result<Taped, String> {
    let recorder = RunRecorder::recording(tape)
        .map_err(|e| format!("opening cassette {}: {e}", tape.display()))?;
    let live =
        Client::new(base_url, api_key).map_err(|e| format!("building provider client: {e}"))?;
    Ok(Taped {
        provider: Arc::new(RecordingProvider::new(
            Arc::new(live) as Arc<dyn ChatProvider>,
            Arc::clone(&recorder),
        )),
        recorder,
    })
}

/// Serve a fresh session from `tape`, taping the replay beside it. Takes no
/// key and builds no network client by construction.
pub(crate) fn replaying(tape: &Path) -> Result<Taped, String> {
    let cassette =
        RunCassette::load(tape).map_err(|e| format!("loading cassette {}: {e}", tape.display()))?;
    let provider = ReplayProvider::new(&cassette)
        .map_err(|e| format!("replaying cassette {}: {e}", tape.display()))?;
    let stem = tape
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "run".to_string());
    let extension = tape
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let destination =
        tape.with_file_name(format!("{stem}.replay-{}{extension}", ulid::Ulid::new()));
    let recorder = RunRecorder::replaying(&destination, &cassette)
        .map_err(|e| format!("opening cassette {}: {e}", destination.display()))?;
    Ok(Taped {
        provider: Arc::new(RecordingProvider::new(
            Arc::new(provider) as Arc<dyn ChatProvider>,
            Arc::clone(&recorder),
        )),
        recorder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_replay_together_are_refused() {
        let err = pick_tape(Some("/tmp/a.jsonl"), Some("/tmp/b.jsonl")).unwrap_err();
        assert!(err.contains("cannot be combined"), "{err}");
    }

    #[test]
    fn resume_and_replay_together_are_refused() {
        let err = check_resume_replay(Some("/tmp/s.jsonl"), Some("/tmp/b.jsonl")).unwrap_err();
        assert!(err.contains("replay starts a fresh session"), "{err}");
        assert!(check_resume_replay(Some("/tmp/s.jsonl"), None).is_ok());
        assert!(check_resume_replay(None, Some("/tmp/b.jsonl")).is_ok());
    }

    #[test]
    fn tape_inside_the_project_is_refused() {
        let vault = tempfile::tempdir().unwrap();
        let root = vault.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let inside = root.join("tape.jsonl");
        let err = check_tape_outside_project(&inside, &root).unwrap_err();
        assert!(err.contains("outside the project"), "{err}");
    }

    #[test]
    fn replaying_a_missing_cassette_names_it() {
        let vault = tempfile::tempdir().unwrap();
        let missing = vault.path().join("nope.jsonl");
        let err = replaying(&missing).unwrap_err();
        assert!(err.contains("nope.jsonl"), "{err}");
    }
}
