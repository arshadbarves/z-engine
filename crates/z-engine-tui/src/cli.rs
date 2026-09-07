//! Argument surface: parse argv into [`Args`], or explain the refusal.
//!
//! Parsing is pure and total — no environment, no filesystem, no exits —
//! so every rejection this binary can produce is a value a test can
//! assert on. Combinations that cannot mean anything (recording a
//! replay, taping an interactive session, replaying under a different
//! model) are refused here rather than half-honoured later.

use std::path::{Path, PathBuf};

use thiserror::Error;

#[cfg(test)]
mod tests;

/// CLI surface for v0.1 (formalized in v1.0).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project: Option<PathBuf>,
    /// One-shot mode: read task from argv/stdin, stream plain-text events,
    /// exit non-zero on failure. Developer/acceptance flag (spec §9 v1.0
    /// formalizes it; recorded in docs/deviations.md).
    pub headless_task: Option<String>,
    /// Headless companion: auto-approve every gated action (unsafe
    /// convenience for scripted acceptance runs).
    pub auto_approve: bool,
    /// Starting permission mode (default|accept-edits|plan).
    pub permission_mode: Option<String>,
    /// Open the session picker at startup.
    pub resume: bool,
    /// Resume a specific session by ULID (or path).
    pub session: Option<String>,
    /// Evidence-gated mode: reads mint evidence, mutations need a scoped
    /// work order, and a turn ends only when verification says so.
    pub guarded: bool,
    /// Record the whole run — provider traffic, gate rulings, evidence,
    /// verdict, cost — onto this cassette.
    pub record_run: Option<PathBuf>,
    /// Serve the run from this cassette instead of a provider.
    pub replay_run: Option<PathBuf>,
    /// Write the run's metrics as JSON here once the run is over.
    pub metrics_out: Option<PathBuf>,
    /// Wall-clock ceiling for a headless run, in seconds. `None` uses the
    /// runner's default, which is finite either way.
    pub timeout_secs: Option<u64>,
    /// `--help` was asked for; the caller prints usage and exits 0.
    pub help: bool,
}

/// Why an invocation cannot be honoured, in the words the user needs.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliError {
    #[error("{0} needs a value")]
    MissingValue(String),
    #[error("unknown argument: {0}")]
    Unknown(String),
    #[error(
        "--record-run and --replay-run cannot be combined: a run is either \
         served by a provider or by a cassette. Replay writes its own tape \
         next to the one it reads."
    )]
    RecordAndReplay,
    #[error(
        "{0} needs --headless: a cassette is a whole run from one task to \
         its verdict, which an interactive session does not have. Use \
         --headless \"task\" {0} <path>."
    )]
    NeedsHeadless(String),
    #[error(
        "--metrics-out needs a cassette: metrics are read back from the \
         tape. Add --record-run <path> (or --replay-run <path>)."
    )]
    MetricsWithoutTape,
    #[error(
        "--replay-run cannot be combined with {0}: the cassette already \
         fixes what was asked and who answered. Drop {0} to replay the \
         recorded run."
    )]
    ReplayOverride(String),
    #[error(
        "--timeout needs a positive whole number of seconds, not {0:?}: a \
         run with no bound is a CI job that never ends."
    )]
    BadTimeout(String),
    #[error(
        "{flag} must point outside the project: {path} is inside {root}, \
         and a guarded run treats every change under its root as work to \
         account for. Put the tape somewhere else."
    )]
    TapeInsideProject {
        flag: String,
        path: String,
        root: String,
    },
}

pub const USAGE: &str = "\
zengine - personal TUI coding agent

USAGE:
  zengine [--model M] [--base-url URL] [--project DIR]
          [--resume | --session ULID] [--guarded]
          [--headless \"task\" | --headless < task.txt] [--auto-approve]

GUARDED RUNS (headless):
  --guarded             evidence-gated mode: reads mint evidence, edits need
                        a scoped work order, and verification ends the turn
  --record-run PATH     record the run (traffic, gates, evidence, verdict)
  --replay-run PATH     serve the run from a cassette: no network, no API key
  --metrics-out PATH    write the run's metrics as JSON
  --timeout SECS        bound the whole run (default 1800s); a run with no
                        verdict by then exits non-zero

Cassettes must live outside the project directory.";

/// Parse argv (already stripped of argv[0]).
pub fn parse(argv: &[String]) -> Result<Args, CliError> {
    let mut args = Args::default();
    let mut i = 0usize;
    while i < argv.len() {
        let value = |i: &mut usize, flag: &str| -> Result<String, CliError> {
            *i += 1;
            argv.get(*i)
                .cloned()
                .ok_or_else(|| CliError::MissingValue(flag.to_string()))
        };
        match argv[i].as_str() {
            "--model" => args.model = Some(value(&mut i, "--model")?),
            "--base-url" => args.base_url = Some(value(&mut i, "--base-url")?),
            "--project" => args.project = Some(PathBuf::from(value(&mut i, "--project")?)),
            "--headless" => {
                // Task = following words up to the next --flag; none ⇒ stdin.
                let mut parts: Vec<String> = Vec::new();
                while i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                    i += 1;
                    parts.push(argv[i].clone());
                }
                args.headless_task = Some(parts.join(" "));
            }
            "--auto-approve" => args.auto_approve = true,
            "--resume" => args.resume = true,
            "--permission-mode" => args.permission_mode = Some(value(&mut i, "--permission-mode")?),
            "--session" => args.session = Some(value(&mut i, "--session")?),
            "--guarded" => args.guarded = true,
            "--record-run" => args.record_run = Some(PathBuf::from(value(&mut i, "--record-run")?)),
            "--replay-run" => args.replay_run = Some(PathBuf::from(value(&mut i, "--replay-run")?)),
            "--metrics-out" => {
                args.metrics_out = Some(PathBuf::from(value(&mut i, "--metrics-out")?))
            }
            "--timeout" => {
                let raw = value(&mut i, "--timeout")?;
                let secs = raw
                    .parse::<u64>()
                    .ok()
                    .filter(|s| *s > 0)
                    .ok_or_else(|| CliError::BadTimeout(raw.clone()))?;
                args.timeout_secs = Some(secs);
            }
            "--help" | "-h" => args.help = true,
            other => return Err(CliError::Unknown(other.to_string())),
        }
        i += 1;
    }
    if args.help {
        return Ok(args);
    }
    args.check()?;
    Ok(args)
}

impl Args {
    /// Reject the combinations that cannot mean anything.
    fn check(&self) -> Result<(), CliError> {
        if self.record_run.is_some() && self.replay_run.is_some() {
            return Err(CliError::RecordAndReplay);
        }
        if self.headless_task.is_none() {
            for (flag, present) in [
                ("--record-run", self.record_run.is_some()),
                ("--replay-run", self.replay_run.is_some()),
                ("--metrics-out", self.metrics_out.is_some()),
                ("--timeout", self.timeout_secs.is_some()),
            ] {
                if present {
                    return Err(CliError::NeedsHeadless(flag.to_string()));
                }
            }
        }
        if self.metrics_out.is_some() && self.record_run.is_none() && self.replay_run.is_none() {
            return Err(CliError::MetricsWithoutTape);
        }
        if self.replay_run.is_some() {
            for (flag, present) in [
                ("--model", self.model.is_some()),
                ("--base-url", self.base_url.is_some()),
            ] {
                if present {
                    return Err(CliError::ReplayOverride(flag.to_string()));
                }
            }
        }
        Ok(())
    }

    /// Refuse tapes that would be written inside the audited workspace.
    ///
    /// Needs the resolved project root, so it is a separate step from
    /// parsing; the caller runs it once the root is known.
    pub fn check_tape_paths(&self, project_root: &Path) -> Result<(), CliError> {
        let root = anchor(project_root);
        for (flag, path) in [
            ("--record-run", self.record_run.as_ref()),
            ("--replay-run", self.replay_run.as_ref()),
            ("--metrics-out", self.metrics_out.as_ref()),
        ] {
            let Some(path) = path else { continue };
            let absolute = if path.is_absolute() {
                path.clone()
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            };
            if anchor(&absolute).starts_with(&root) {
                return Err(CliError::TapeInsideProject {
                    flag: flag.to_string(),
                    path: path.display().to_string(),
                    root: root.display().to_string(),
                });
            }
        }
        Ok(())
    }

    /// Does this invocation need provider credentials?
    ///
    /// A replayed run is served entirely from its cassette, so it must
    /// never read a key, let alone send one anywhere.
    pub fn needs_api_key(&self) -> bool {
        self.replay_run.is_none()
    }
}

/// Resolve `path` as far as the filesystem can, keeping the part that does
/// not exist yet.
///
/// Both sides of an "is this inside the project?" question must be
/// anchored the same way: a macOS temp dir reaches its real location
/// through a symlink (`/var` → `/private/var`), so comparing a resolved
/// root against an unresolved tape would call a tape inside the project
/// outside it — exactly for the not-yet-created directories a first run
/// makes.
fn anchor(path: &Path) -> PathBuf {
    let mut pending: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path.to_path_buf();
    loop {
        if let Ok(real) = std::fs::canonicalize(&cursor) {
            let mut resolved = real;
            resolved.extend(pending.iter().rev());
            return resolved;
        }
        let (Some(parent), Some(name)) = (cursor.parent(), cursor.file_name()) else {
            return path.to_path_buf();
        };
        pending.push(name.to_owned());
        let parent = parent.to_path_buf();
        if parent.as_os_str().is_empty() {
            return path.to_path_buf();
        }
        cursor = parent;
    }
}
