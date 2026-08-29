//! `EvidenceLedger`: an append-only, human-readable `evidence.jsonl`
//! transcript of every [`EvidenceRecord`] captured during a run. Records
//! are appended one flushed JSON object per line and replayed in the
//! same order later (e.g. by work-order validation and replay).
//!
//! Unlike the session transcript, which tolerates a torn trailing write
//! by skipping it, evidence backs write authorization: a corrupt record
//! must never be silently dropped, so `read_all` fails loudly instead.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::error::EvidenceError;
use super::record::EvidenceRecord;

const LEDGER_FILE_NAME: &str = "evidence.jsonl";

/// Append-only handle onto one run's `evidence.jsonl`.
#[derive(Debug)]
pub struct EvidenceLedger {
    file: File,
    path: PathBuf,
}

impl EvidenceLedger {
    /// Open (creating if needed) the evidence ledger under `root` (e.g.
    /// `.z-engine/runs/<run-id>`). Reopening an existing ledger appends
    /// to its prior contents rather than truncating them.
    pub fn open(root: &Path) -> Result<Self, EvidenceError> {
        std::fs::create_dir_all(root).map_err(|source| EvidenceError::LedgerOpen {
            path: root.to_path_buf(),
            source,
        })?;
        let path = root.join(LEDGER_FILE_NAME);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| EvidenceError::LedgerOpen {
                path: path.clone(),
                source,
            })?;
        Ok(Self { file, path })
    }

    /// Append one record as a single flushed line. Takes `&self` (not
    /// `&mut self`): appends never need exclusive access, since each
    /// write is a self-contained line and `File` supports concurrent
    /// appends via `&File`.
    pub fn append(&self, record: &EvidenceRecord) -> Result<(), EvidenceError> {
        let mut line =
            serde_json::to_string(record).map_err(|source| EvidenceError::Serialize { source })?;
        line.push('\n');
        let mut file = &self.file;
        file.write_all(line.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|source| EvidenceError::Append {
                path: self.path.clone(),
                source,
            })
    }

    /// Read every record in append order. Any malformed line — bad JSON
    /// or a bad blob handle — fails the whole read rather than being
    /// skipped, since silently dropping evidence would make writes look
    /// authorized when they are not.
    pub fn read_all(&self) -> Result<Vec<EvidenceRecord>, EvidenceError> {
        let file = File::open(&self.path).map_err(|source| EvidenceError::LedgerRead {
            path: self.path.clone(),
            source,
        })?;
        let mut out = Vec::new();
        for (idx, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|source| EvidenceError::LedgerRead {
                path: self.path.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record: EvidenceRecord =
                serde_json::from_str(&line).map_err(|source| EvidenceError::CorruptRecord {
                    path: self.path.clone(),
                    line: idx + 1,
                    source,
                })?;
            out.push(record);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::blob::BlobHandle;
    use super::*;

    fn fixture(label: &str) -> EvidenceRecord {
        EvidenceRecord::new(
            format!("src/{label}.rs"),
            None,
            "0".repeat(64),
            BlobHandle::of(label.as_bytes()),
            "read_file",
            "working-tree",
        )
    }

    #[test]
    fn ledger_is_append_only_and_ordered() {
        // `dir` must outlive the ledger: `tempdir().path()` used inline
        // would drop (and delete) the TempDir at the end of the `let
        // ledger = ...` statement, so the second `append` would write
        // into an already-unlinked file.
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        ledger.append(&fixture("a")).unwrap();
        ledger.append(&fixture("b")).unwrap();
        let records = ledger.read_all().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].path, "src/a.rs");
        assert_eq!(records[1].path, "src/b.rs");
    }

    #[test]
    fn ledger_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let ledger = EvidenceLedger::open(dir.path()).unwrap();
            ledger.append(&fixture("a")).unwrap();
        }
        let reopened = EvidenceLedger::open(dir.path()).unwrap();
        reopened.append(&fixture("b")).unwrap();
        let records = reopened.read_all().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].path, "src/a.rs");
        assert_eq!(records[1].path, "src/b.rs");
    }

    #[test]
    fn read_all_fails_loudly_on_corrupt_json_line() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        ledger.append(&fixture("a")).unwrap();
        // Simulate a torn/corrupted append landing on disk.
        let mut raw = OpenOptions::new()
            .append(true)
            .open(dir.path().join(LEDGER_FILE_NAME))
            .unwrap();
        raw.write_all(b"{not valid json\n").unwrap();

        let err = ledger.read_all().unwrap_err();
        assert!(matches!(err, EvidenceError::CorruptRecord { line: 2, .. }));
    }

    #[test]
    fn read_all_fails_loudly_on_malformed_blob_handle() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        let mut raw = OpenOptions::new()
            .append(true)
            .open(dir.path().join(LEDGER_FILE_NAME))
            .unwrap();
        raw.write_all(
            br#"{"id":"x","path":"src/a.rs","line_range":null,"file_hash":"0","blob":"not-a-hash","method":"read_file","revision":"working-tree"}
"#,
        )
        .unwrap();

        let err = ledger.read_all().unwrap_err();
        assert!(matches!(err, EvidenceError::CorruptRecord { line: 1, .. }));
    }

    #[test]
    fn empty_ledger_reads_as_no_records() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        assert_eq!(ledger.read_all().unwrap(), Vec::new());
    }
}
