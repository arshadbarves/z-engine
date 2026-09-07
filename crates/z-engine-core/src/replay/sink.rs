//! Where a recorder's entries go.
//!
//! Recording is one append per fact, flushed, never rewritten — the same
//! shape as the evidence ledger, for the same reason: a record that can
//! be edited after the fact proves nothing.
//!
//! The sink is a trait rather than the file directly so that "the write
//! failed" is a case the recorder can be *shown*, not just reasoned
//! about. Fail-closed behaviour that is never exercised is a claim, not
//! a property.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::entry::CassetteEntry;
use super::error::ReplayError;

/// Somewhere a cassette entry can be appended.
pub trait EntrySink: Send + Sync + std::fmt::Debug {
    /// Append one entry durably. An `Err` means the entry is *not*
    /// recorded; the caller must treat the tape as no longer whole.
    fn append(&self, entry: &CassetteEntry) -> Result<(), ReplayError>;
}

/// Append-only writer onto one run's cassette file.
#[derive(Debug)]
pub struct CassetteWriter {
    file: File,
    path: PathBuf,
}

impl CassetteWriter {
    /// Create `path` for a new run. An existing file is refused rather
    /// than appended to or truncated: a cassette is exactly one run, and
    /// a tape holding two would replay as neither.
    pub fn create(path: &Path) -> Result<Self, ReplayError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ReplayError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(path)
            .map_err(|source| match source.kind() {
                std::io::ErrorKind::AlreadyExists => ReplayError::Exists {
                    path: path.to_path_buf(),
                },
                _ => ReplayError::Io {
                    path: path.to_path_buf(),
                    source,
                },
            })?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }
}

impl EntrySink for CassetteWriter {
    /// Append one entry as a single flushed line.
    fn append(&self, entry: &CassetteEntry) -> Result<(), ReplayError> {
        let mut line =
            serde_json::to_string(entry).map_err(|source| ReplayError::Serialize { source })?;
        line.push('\n');
        let mut file = &self.file;
        file.write_all(line.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|source| ReplayError::Io {
                path: self.path.clone(),
                source,
            })
    }
}

/// A sink that refuses everything, so fail-closed behaviour is a thing
/// that happens in a test rather than a thing asserted about.
#[cfg(test)]
#[derive(Debug)]
pub(super) struct BrokenSink;

#[cfg(test)]
impl EntrySink for BrokenSink {
    fn append(&self, _entry: &CassetteEntry) -> Result<(), ReplayError> {
        Err(ReplayError::Io {
            path: std::path::PathBuf::from("/dev/full"),
            source: std::io::Error::other("no space left on device"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::RunCassette;
    use crate::replay::entry::{PromptHash, content_hash};

    fn prompt(sequence: u64) -> CassetteEntry {
        CassetteEntry::Prompt(PromptHash {
            sequence,
            hash: content_hash(b"prefix"),
        })
    }

    #[test]
    fn each_entry_is_one_flushed_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer.append(&prompt(0)).unwrap();
        writer.append(&prompt(1)).unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body.lines().count(), 2);
        assert_eq!(RunCassette::load(&path).unwrap().prompt_hashes().len(), 2);
    }

    #[test]
    fn a_cassette_records_exactly_one_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        CassetteWriter::create(&path).unwrap();
        let err = CassetteWriter::create(&path).unwrap_err();
        assert!(matches!(err, ReplayError::Exists { .. }), "{err}");
    }
}
