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
}
