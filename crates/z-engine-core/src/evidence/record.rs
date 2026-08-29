//! `EvidenceRecord`: one durable proof that a specific range of a
//! specific repository path was actually read, at a specific revision,
//! grounding later guarded writes in real observations rather than
//! model claims.

use serde::{Deserialize, Serialize};

use super::blob::BlobHandle;

/// One piece of content-addressed evidence.
///
/// Fields are intentionally plain data (no timestamps) so records stay
/// deterministic and reproducible across runs and replays; freshness is
/// judged by comparing `file_hash`/`revision` against the current
/// working tree, not by wall-clock time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// Stable identifier (e.g. a ULID) referenced by later work orders.
    pub id: String,
    /// Repository-relative path the evidence was captured from
    /// (canonical, forward-slash separated).
    pub path: String,
    /// Inclusive 1-based `(start, end)` line range actually returned to
    /// the caller; `None` means the whole file was captured.
    pub line_range: Option<(u32, u32)>,
    /// SHA-256 (hex) of the complete file's bytes at capture time, used
    /// to detect staleness against the working tree.
    pub file_hash: String,
    /// Content-addressed handle for the bytes actually returned (the
    /// range, or the whole file when `line_range` is `None`).
    pub blob: BlobHandle,
    /// How the evidence was acquired, e.g. `"read_file"`.
    pub method: String,
    /// Git HEAD (short or full SHA) at capture time, or
    /// `"working-tree"` when the repository revision could not be
    /// resolved.
    pub revision: String,
}

impl EvidenceRecord {
    /// Construct a record with a fresh ULID identifier.
    pub fn new(
        path: impl Into<String>,
        line_range: Option<(u32, u32)>,
        file_hash: impl Into<String>,
        blob: BlobHandle,
        method: impl Into<String>,
        revision: impl Into<String>,
    ) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            path: path.into(),
            line_range,
            file_hash: file_hash.into(),
            blob,
            method: method.into(),
            revision: revision.into(),
        }
    }
}
