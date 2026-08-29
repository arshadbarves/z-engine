//! Typed failure modes for evidence storage. Corruption must always
//! surface as one of these variants — nothing in this module is allowed
//! to silently drop or skip a bad record or blob.

use std::path::PathBuf;

use super::blob::BlobHandle;

#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    #[error("malformed blob handle {0:?}: expected 64 lowercase hex characters")]
    MalformedHandle(String),

    #[error("failed initializing blob store root at {path}: {source}")]
    Init {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed writing blob {handle} at {path}: {source}")]
    BlobWrite {
        handle: BlobHandle,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed reading blob {handle} at {path}: {source}")]
    BlobRead {
        handle: BlobHandle,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("blob {handle} is missing from the store at {path}")]
    BlobMissing { handle: BlobHandle, path: PathBuf },

    #[error("blob {handle} content does not match its hash (found {actual})")]
    HashMismatch {
        handle: BlobHandle,
        actual: BlobHandle,
    },

    #[error("failed opening evidence ledger at {path}: {source}")]
    LedgerOpen {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed appending an evidence record to {path}: {source}")]
    Append {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed serializing an evidence record: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },

    #[error("failed reading evidence ledger at {path}: {source}")]
    LedgerRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("corrupt evidence record at line {line} of {path}: {source}")]
    CorruptRecord {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
}
