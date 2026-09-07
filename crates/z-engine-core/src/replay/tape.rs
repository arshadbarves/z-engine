//! The tape itself: an append-only JSONL file of [`CassetteEntry`]
//! lines, and the loaded run it reads back as.
//!
//! Same shape as the evidence ledger — one flushed JSON object per line,
//! never rewritten — for the same reason: a record that can be edited
//! after the fact proves nothing. Reading is strict in both directions:
//! a malformed line fails the load rather than being skipped, and
//! creating a cassette over an existing one is refused, because a tape
//! holding two runs replays as neither.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::entry::{
    CassetteEntry, CompletionRecord, EvidenceMint, GateDecision, PromptHash, ProviderExchange,
    RunMetrics, ToolOutcome,
};
use super::error::ReplayError;

/// Append-only writer onto one run's cassette.
#[derive(Debug)]
pub(crate) struct CassetteWriter {
    file: File,
    path: PathBuf,
}

impl CassetteWriter {
    /// Create `path` for a new run. An existing file is refused rather
    /// than appended to or truncated: a cassette is exactly one run, and
    /// a tape holding two would replay as neither.
    pub(crate) fn create(path: &Path) -> Result<Self, ReplayError> {
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

    /// Append one entry as a single flushed line.
    pub(crate) fn append(&self, entry: &CassetteEntry) -> Result<(), ReplayError> {
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

/// One recorded run, loaded whole.
#[derive(Debug, Clone)]
pub struct RunCassette {
    path: PathBuf,
    entries: Vec<CassetteEntry>,
}

impl RunCassette {
    /// Read every entry. A malformed line fails the load rather than
    /// being skipped: a tape with a hole in it would replay as a
    /// different run while looking like the same one.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ReplayError> {
        let path = path.as_ref().to_path_buf();
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
        Ok(Self { path, entries })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entries(&self) -> &[CassetteEntry] {
        &self.entries
    }

    /// Recorded exchanges in request order. Entries land when a stream
    /// *ends*, so append order is not request order; the sequence
    /// allocated when the request was issued is.
    pub fn exchanges(&self) -> Vec<ProviderExchange> {
        let mut out: Vec<ProviderExchange> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Exchange(x) => Some(x.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|x| x.sequence);
        out
    }

    pub fn request_hashes(&self) -> Vec<String> {
        self.exchanges()
            .into_iter()
            .map(|x| x.request_hash)
            .collect()
    }

    pub fn prompt_hashes(&self) -> Vec<String> {
        let mut out: Vec<PromptHash> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Prompt(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|p| p.sequence);
        out.into_iter().map(|p| p.hash).collect()
    }

    pub fn tool_outcomes(&self) -> Vec<ToolOutcome> {
        let mut out: Vec<ToolOutcome> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Tool(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|t| t.sequence);
        out
    }

    pub fn gate_decisions(&self) -> Vec<GateDecision> {
        let mut out: Vec<GateDecision> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Gate(g) => Some(g.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|g| g.sequence);
        out
    }

    pub fn evidence_ids(&self) -> Vec<EvidenceMint> {
        let mut out: Vec<EvidenceMint> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Evidence(m) => Some(m.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|m| m.sequence);
        out
    }

    /// The last completion recorded — a run is judged on where it ended.
    pub fn completion(&self) -> Option<CompletionRecord> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Completion(c) => Some(c.clone()),
                _ => None,
            })
            .max_by_key(|c| c.sequence)
    }

    pub fn manifest_hash(&self) -> Option<String> {
        self.completion().map(|c| c.manifest_hash)
    }

    pub fn diff_hash(&self) -> Option<String> {
        self.completion().and_then(|c| c.diff_hash)
    }

    /// Metrics are appended once per turn; the last line is the run's.
    pub fn metrics(&self) -> Option<RunMetrics> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Metrics(m) => Some(m.clone()),
                _ => None,
            })
            .next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::entry::{GateKind, canonical_request, content_hash};
    use z_engine_provider::{ChatMessage, ChatRequest, StreamEvent};

    fn exchange(sequence: u64, model: &str) -> CassetteEntry {
        let request = ChatRequest::new(model, vec![ChatMessage::user("hi")]);
        let (_, hash) = canonical_request(&request).unwrap();
        CassetteEntry::Exchange(ProviderExchange {
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
            exchange(0, "m"),
            CassetteEntry::Prompt(PromptHash {
                sequence: 0,
                hash: content_hash(b"prefix"),
            }),
            CassetteEntry::Tool(ToolOutcome {
                sequence: 0,
                name: "read_file".into(),
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
    }

    /// Streams finish out of order under concurrency; request order is
    /// the order the requests were *issued* in.
    #[test]
    fn exchanges_replay_in_issue_order_not_append_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer.append(&exchange(1, "second")).unwrap();
        writer.append(&exchange(0, "first")).unwrap();

        let cassette = RunCassette::load(&path).unwrap();
        let models: Vec<String> = cassette
            .exchanges()
            .into_iter()
            .map(|x| x.request.model)
            .collect();
        assert_eq!(models, ["first", "second"]);
    }

    #[test]
    fn a_corrupt_line_fails_the_load_instead_of_being_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer.append(&exchange(0, "m")).unwrap();
        drop(writer);
        let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
        raw.write_all(b"{not json\n").unwrap();

        let err = RunCassette::load(&path).unwrap_err();
        assert!(matches!(err, ReplayError::Corrupt { line: 2, .. }), "{err}");
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
