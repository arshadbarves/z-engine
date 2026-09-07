//! Why a cassette could not be written, read, or honored.
//!
//! Every variant is a refusal rather than a degradation: a tape that
//! cannot be trusted is more dangerous than no tape at all, because a
//! run replayed from it would look like the run it is not.

use std::path::PathBuf;

/// Why a cassette could not be written, read, or honored.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("cassette {path} already exists; one cassette records exactly one run")]
    Exists { path: PathBuf },
    #[error("cassette {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cassette {path} line {line} is not a valid entry: {source}")]
    Corrupt {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("cassette entry could not be serialized: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "replay has no recorded evidence id for {path} (read #{ordinal}): this run is reading \
         something the recorded run did not"
    )]
    EvidenceExhausted { path: String, ordinal: usize },
    #[error(
        "cassette {path} is not a whole run: its {entry} entry was lost while recording ({detail})"
    )]
    Incomplete {
        path: PathBuf,
        entry: String,
        detail: String,
    },
    #[error(
        "cassette {path} lane {lane} jumps from request #{expected} to #{found}: a tape with a \
         hole in it replays as a run that never happened"
    )]
    SequenceGap {
        path: PathBuf,
        lane: String,
        expected: u64,
        found: u64,
    },
}
