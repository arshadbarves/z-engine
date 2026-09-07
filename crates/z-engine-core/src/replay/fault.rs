//! Why a run's record is not whole, and where that is written down.
//!
//! An append that fails leaves a cassette that is short of the run it
//! claims to be — which is worse than no cassette at all, because it
//! replays as a run that never happened while looking like one that did.
//!
//! So a lost entry is persisted twice over. It is appended to the tape
//! best-effort (the same failure usually eats that too), and it is
//! written to a marker file *beside* the tape, in a separate `write`
//! to a separate path. [`super::RunCassette::load`] refuses any cassette
//! carrying a marker, so the failure cannot be lost by being forgotten.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::error::ReplayError;

/// Suffix of the marker written beside a cassette that lost an entry.
const MARKER_SUFFIX: &str = ".incomplete";

/// Why a run's record is not whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingFault {
    /// The kind of entry that was lost (`"exchange"`, `"tool"`, …).
    /// Named `kind` rather than `entry` because a cassette line is
    /// tagged `entry`, and a fault is one of those lines.
    pub kind: String,
    /// Why it could not be recorded.
    pub detail: String,
}

impl std::fmt::Display for RecordingFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} entry was lost: {}", self.kind, self.detail)
    }
}

/// Where the marker for `cassette` lives.
pub(super) fn marker_path(cassette: &Path) -> PathBuf {
    let mut name = cassette.as_os_str().to_os_string();
    name.push(MARKER_SUFFIX);
    PathBuf::from(name)
}

/// Mark `cassette` as not whole. Written whole rather than appended: a
/// half-written marker would be one more thing to distrust.
pub(super) fn mark_incomplete(cassette: &Path, fault: &RecordingFault) -> Result<(), ReplayError> {
    let path = marker_path(cassette);
    let body = serde_json::to_string(fault).map_err(|source| ReplayError::Serialize { source })?;
    std::fs::write(&path, body).map_err(|source| ReplayError::Io { path, source })
}

/// The fault marking `cassette`, if it carries one.
///
/// A marker that cannot be parsed still counts as a fault — something
/// wrote it, and the one thing it could have been saying is that this
/// tape is not to be trusted.
pub(super) fn read_marker(cassette: &Path) -> Option<RecordingFault> {
    let body = std::fs::read_to_string(marker_path(cassette)).ok()?;
    Some(
        serde_json::from_str(&body).unwrap_or_else(|_| RecordingFault {
            kind: "unknown".into(),
            detail: "the recording was marked incomplete, but the marker is unreadable".into(),
        }),
    )
}

/// The first thing a run failed to record, and the fact of it on disk.
///
/// Only the first loss is kept: later ones are downstream of it and
/// would bury the cause.
#[derive(Debug)]
pub(super) struct FaultLog {
    cassette: PathBuf,
    first: Mutex<Option<RecordingFault>>,
}

impl FaultLog {
    pub(super) fn beside(cassette: &Path) -> Self {
        Self {
            cassette: cassette.to_path_buf(),
            first: Mutex::new(None),
        }
    }

    /// What condemned this run, if anything has.
    pub(super) fn get(&self) -> Option<RecordingFault> {
        self.first.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Remember `fault` and write it beside the tape. Returns it only
    /// when it is the first, so the caller knows it still owes the tape
    /// a copy.
    pub(super) fn condemn(&self, fault: RecordingFault) -> Option<RecordingFault> {
        let mut slot = self.first.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return None;
        }
        tracing::error!(entry = %fault.kind, detail = %fault.detail, "cassette: entry lost");
        // Beside the tape first: the write that lost the entry is the
        // one least likely to accept another.
        if let Err(err) = mark_incomplete(&self.cassette, &fault) {
            tracing::error!(%err, "cassette: the incomplete marker could not be written either");
        }
        *slot = Some(fault.clone());
        Some(fault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault() -> RecordingFault {
        RecordingFault {
            kind: "exchange".into(),
            detail: "disk full".into(),
        }
    }

    #[test]
    fn a_fault_names_the_entry_it_lost() {
        assert_eq!(fault().to_string(), "exchange entry was lost: disk full");
    }

    /// The marker sits beside the tape rather than inside it, because
    /// the failure that lost an entry usually cannot write another one.
    #[test]
    fn the_marker_is_a_separate_file_beside_the_cassette() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = dir.path().join("run.jsonl");
        assert!(read_marker(&cassette).is_none());

        mark_incomplete(&cassette, &fault()).unwrap();
        assert_eq!(
            marker_path(&cassette),
            dir.path().join("run.jsonl.incomplete")
        );
        assert_eq!(read_marker(&cassette), Some(fault()));
    }

    #[test]
    fn only_the_first_loss_is_kept_and_it_reaches_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = dir.path().join("run.jsonl");
        let log = FaultLog::beside(&cassette);

        assert_eq!(
            log.condemn(fault()),
            Some(fault()),
            "the first is the tape's"
        );
        assert_eq!(log.get(), Some(fault()));
        assert_eq!(read_marker(&cassette), Some(fault()));

        let later = RecordingFault {
            kind: "tool".into(),
            detail: "downstream of the first".into(),
        };
        assert_eq!(
            log.condemn(later),
            None,
            "later losses do not bury the cause"
        );
        assert_eq!(log.get(), Some(fault()));
    }

    #[test]
    fn an_unreadable_marker_still_condemns_the_tape() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = dir.path().join("run.jsonl");
        std::fs::write(marker_path(&cassette), "{not json").unwrap();
        let found = read_marker(&cassette).expect("a marker at all is a fault");
        assert_eq!(found.kind, "unknown");
    }
}
