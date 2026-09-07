//! The tape itself: an append-only JSONL file of [`CassetteEntry`]
//! lines, and the loaded run it reads back as.
//!
//! Reading is strict in every direction. A malformed line fails the load
//! rather than being skipped; a cassette whose recorder lost an entry
//! fails the load rather than passing for whole (see [`super::fault`]).
//! A tape that can be half-read is a tape that replays as a run that
//! never happened.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::entry::CassetteEntry;
use super::error::ReplayError;
use super::fault::{RecordingFault, read_marker};

/// One recorded run, loaded whole.
#[derive(Debug, Clone)]
pub struct RunCassette {
    path: PathBuf,
    entries: Vec<CassetteEntry>,
}

impl RunCassette {
    /// Read every entry.
    ///
    /// Refuses a cassette its recorder marked incomplete, and refuses a
    /// malformed line rather than skipping it: either way the tape would
    /// otherwise replay as a different run while looking like the same
    /// one.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ReplayError> {
        let path = path.as_ref().to_path_buf();
        if let Some(fault) = read_marker(&path) {
            return Err(ReplayError::Incomplete {
                path,
                entry: fault.kind,
                detail: fault.detail,
            });
        }
        let file = File::open(&path).map_err(|source| ReplayError::Io {
            path: path.clone(),
            source,
        })?;
        let mut entries = Vec::new();
        for (idx, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|source| ReplayError::Io {
                path: path.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            entries.push(
                serde_json::from_str(&line).map_err(|source| ReplayError::Corrupt {
                    path: path.clone(),
                    line: idx + 1,
                    source,
                })?,
            );
        }
        let loaded = Self { path, entries };
        // A recorder that managed to append its own fault condemns the
        // tape just as loudly as the marker beside it does.
        match loaded.fault() {
            Some(fault) => Err(ReplayError::Incomplete {
                path: loaded.path,
                entry: fault.kind,
                detail: fault.detail,
            }),
            None => Ok(loaded),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entries(&self) -> &[CassetteEntry] {
        &self.entries
    }

    /// The first recording failure the run taped, if it taped one.
    pub fn fault(&self) -> Option<RecordingFault> {
        self.entries.iter().find_map(|e| match e {
            CassetteEntry::Fault(f) => Some(f.clone()),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::entry::{
        CompletionRecord, EvidenceMint, GateDecision, GateKind, PromptHash, ProviderExchange,
        RunMetrics, ToolDisposition, ToolOutcome, canonical_request, content_hash,
    };
    use crate::replay::fault::mark_incomplete;
    use crate::replay::sink::{CassetteWriter, EntrySink};
    use std::io::Write;
    use z_engine_provider::{ChatMessage, ChatRequest, RequestLane, StreamEvent};

    fn exchange(lane: RequestLane, sequence: u64, model: &str) -> CassetteEntry {
        let request = ChatRequest::new(model, vec![ChatMessage::user("hi")]);
        let (_, hash) = canonical_request(&request).unwrap();
        CassetteEntry::Exchange(ProviderExchange {
            lane,
            sequence,
            request_hash: hash,
            request,
            events: vec![StreamEvent::TextDelta("ok".into()), StreamEvent::Done],
            error: None,
        })
    }

    #[test]
    fn entries_round_trip_through_one_jsonl_line_each() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        let entries = vec![
            exchange(RequestLane::MAIN, 0, "m"),
            CassetteEntry::Prompt(PromptHash {
                sequence: 0,
                hash: content_hash(b"prefix"),
            }),
            CassetteEntry::Tool(ToolOutcome {
                sequence: 0,
                name: "read_file".into(),
                disposition: ToolDisposition::Executed,
                ok: true,
                result_hash: content_hash(b"body"),
            }),
            CassetteEntry::Gate(GateDecision {
                sequence: 0,
                kind: GateKind::Mutation,
                target: "src/lib.rs".into(),
                allowed: false,
                reason: Some("no work order".into()),
            }),
            CassetteEntry::Evidence(EvidenceMint {
                sequence: 0,
                path: "src/lib.rs".into(),
                range: Some((1, 12)),
                id: "01ABC".into(),
            }),
            CassetteEntry::Completion(CompletionRecord {
                sequence: 0,
                manifest_hash: content_hash(b"manifest"),
                diff_hash: Some(content_hash(b"diff")),
                verdict: "complete".into(),
            }),
            CassetteEntry::Metrics(RunMetrics {
                model_id: "m".into(),
                input_tokens: 10,
                output_tokens: 5,
                turns: 1,
                tool_calls: 1,
                wall_time_ms: 42,
                outcome: "completed".into(),
            }),
        ];
        for entry in &entries {
            writer.append(entry).unwrap();
        }

        let cassette = RunCassette::load(&path).unwrap();
        assert_eq!(cassette.entries(), entries);
        assert_eq!(cassette.request_hashes().len(), 1);
        assert_eq!(cassette.tool_outcomes()[0].name, "read_file");
        assert!(!cassette.gate_decisions()[0].allowed);
        assert_eq!(cassette.evidence_ids()[0].id, "01ABC");
        assert_eq!(cassette.diff_hash(), Some(content_hash(b"diff")));
        assert_eq!(cassette.metrics().unwrap().turns, 1);
        assert!(cassette.fault().is_none());
    }

    #[test]
    fn a_corrupt_line_fails_the_load_instead_of_being_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer.append(&exchange(RequestLane::MAIN, 0, "m")).unwrap();
        drop(writer);
        let mut raw = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        raw.write_all(b"{not json\n").unwrap();

        let err = RunCassette::load(&path).unwrap_err();
        assert!(matches!(err, ReplayError::Corrupt { line: 2, .. }), "{err}");
    }

    /// A tape whose recorder lost an entry is short of the run it claims
    /// to be, so it does not get to be loaded at all.
    #[test]
    fn a_cassette_marked_incomplete_is_refused_rather_than_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer.append(&exchange(RequestLane::MAIN, 0, "m")).unwrap();
        RunCassette::load(&path).expect("whole until marked");

        mark_incomplete(
            &path,
            &RecordingFault {
                kind: "tool".into(),
                detail: "disk full".into(),
            },
        )
        .unwrap();
        let err = RunCassette::load(&path).unwrap_err();
        assert!(
            matches!(&err, ReplayError::Incomplete { entry, .. } if entry == "tool"),
            "{err}"
        );
    }

    /// The marker can be lost along with the entry; a fault the recorder
    /// did manage to append has to condemn the tape on its own.
    #[test]
    fn a_taped_fault_alone_is_enough_to_refuse_the_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer.append(&exchange(RequestLane::MAIN, 0, "m")).unwrap();
        writer
            .append(&CassetteEntry::Fault(RecordingFault {
                kind: "exchange".into(),
                detail: "disk full".into(),
            }))
            .unwrap();

        let err = RunCassette::load(&path).unwrap_err();
        assert!(
            matches!(&err, ReplayError::Incomplete { entry, .. } if entry == "exchange"),
            "{err}"
        );
    }
}
