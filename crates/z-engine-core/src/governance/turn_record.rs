//! What a guarded run currently treats as *already accounted for*, and
//! the only two ways that line is allowed to move.
//!
//! A run is not one turn. The first turn's completion check compiles the
//! workspace, and compiling writes: `cargo` refreshes the lockfile,
//! test binaries land under `target/`. If "the workspace as the run
//! started" were the only reference point, every later turn would open
//! with changes no agent made and no tool logged, and a guarded session
//! would be able to complete exactly once. Equally, refreshing that
//! reference point at the start of each turn would bless whatever the
//! previous turn was refused for.
//!
//! So the record moves at exactly one moment — a turn that *verified* —
//! and otherwise absorbs only what the harness's own checks did:
//!
//! | how the turn ended | baseline | mutation log |
//! |---|---|---|
//! | verified | becomes the workspace the checks left behind | cleared |
//! | blocked / stopped / failed | keeps everything the turn must still answer for, plus the harness's own writes | kept |
//!
//! Both halves live under one lock because they are one fact: "what this
//! run has already been judged on". Reading them separately could see a
//! baseline from before a settlement beside a log from after it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use super::plan::{ChangeState, MutationRecord, WorkspaceChange};
use super::snapshot::WorkspaceSnapshot;

/// The run's own record of itself could not be read.
///
/// Reported rather than swallowed: an empty record and an unreadable one
/// look identical to a caller that defaults, and one of them means "this
/// run changed nothing" while the other means "this run cannot say what
/// it changed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this run's record of what it started from and what it changed is unreadable")]
pub struct TurnRecordUnavailable;

/// The state a guarded turn is judged against, and the writes it made.
#[derive(Debug, Default)]
struct Settled {
    /// The tree as of the last settlement; `None` when nothing captured
    /// one, which verification treats as a reason to block a changed
    /// workspace rather than as an empty change set.
    baseline: Option<WorkspaceSnapshot>,
    /// Path to the hash of the bytes the last authorized write left there.
    mutated: BTreeMap<PathBuf, String>,
}

/// Shared, interior-mutable holder for the settled state above.
#[derive(Debug, Default)]
pub struct TurnRecord {
    inner: Mutex<Settled>,
}

impl TurnRecord {
    /// A record that starts from `baseline` — the workspace as the
    /// guarded run first found it.
    pub fn with_baseline(baseline: WorkspaceSnapshot) -> Self {
        Self {
            inner: Mutex::new(Settled {
                baseline: Some(baseline),
                mutated: BTreeMap::new(),
            }),
        }
    }

    /// The workspace this turn is judged against, if one was captured.
    pub fn baseline(&self) -> Result<Option<WorkspaceSnapshot>, TurnRecordUnavailable> {
        Ok(self.locked()?.baseline.clone())
    }

    /// Record that `repo_relative` was changed under this turn, and the
    /// hash of the bytes the tool left there. A later write to the same
    /// path replaces the hash: what completion authorizes is the state
    /// the turn finished in.
    pub fn note_mutation(&self, repo_relative: PathBuf, content_hash: String) {
        if let Ok(mut settled) = self.inner.lock() {
            settled.mutated.insert(repo_relative, content_hash);
        }
    }

    /// Every change this turn authorized, ordered by path.
    pub fn mutations(&self) -> Result<Vec<MutationRecord>, TurnRecordUnavailable> {
        Ok(self
            .locked()?
            .mutated
            .iter()
            .map(|(path, content_hash)| MutationRecord {
                path: path.clone(),
                content_hash: content_hash.clone(),
            })
            .collect())
    }

    /// A turn passed verification: `settled` — the workspace as the
    /// checks left it — is what the next turn starts from, and the
    /// authorized writes it contains have been proven, so the log starts
    /// empty again.
    ///
    /// This is the only place a change the agent made becomes part of the
    /// baseline, and it is reachable only from a complete manifest.
    pub fn settle_verified(&self, settled: WorkspaceSnapshot) -> Result<(), TurnRecordUnavailable> {
        let mut record = self.locked()?;
        record.baseline = Some(settled);
        record.mutated.clear();
        Ok(())
    }

    /// A turn did not pass: everything it must still answer for stays on
    /// the books. Only `harness_writes` — what the harness's own checks
    /// did while the turn was being judged — is absorbed, because no
    /// agent made those and no later turn can fix them.
    ///
    /// A harness write to a path the log already authorized supersedes
    /// the logged hash: the bytes on disk are now the harness's, and the
    /// next audit compares against what is actually there.
    pub fn settle_refused(
        &self,
        harness_writes: &[WorkspaceChange],
    ) -> Result<(), TurnRecordUnavailable> {
        if harness_writes.is_empty() {
            return Ok(());
        }
        let mut record = self.locked()?;
        if let Some(baseline) = record.baseline.as_mut() {
            baseline.absorb(harness_writes);
        }
        for change in harness_writes {
            if let Some(logged) = record.mutated.get_mut(&change.path) {
                match &change.state {
                    ChangeState::Present { content_hash } => *logged = content_hash.clone(),
                    ChangeState::Missing => {
                        record.mutated.remove(&change.path);
                    }
                }
            }
        }
        Ok(())
    }

    fn locked(&self) -> Result<std::sync::MutexGuard<'_, Settled>, TurnRecordUnavailable> {
        self.inner.lock().map_err(|_| TurnRecordUnavailable)
    }

    /// Poison the record the way a panicking tool would, so tests can
    /// exercise the fail-closed path rather than assert it exists.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = self.inner.lock().unwrap();
            panic!("poisoning this run's turn record");
        }));
        std::panic::set_hook(hook);
    }
}

#[cfg(test)]
mod tests;
