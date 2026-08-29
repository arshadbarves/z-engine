//! The governance module's port onto workspace evidence.
//!
//! Work-order admission needs two things it must never re-implement:
//! the canonical identity of a model-supplied path, and whether the
//! evidence captured for that path still matches the bytes on disk.
//! Both already exist behind `ToolCtx` (Task 3), which owns the shared
//! path-identity and hashing logic; this trait is the narrow seam that
//! lets governance ask for them without depending on tool internals —
//! and lets tests supply a fake without touching the filesystem.

use std::path::Path;

use crate::evidence::EvidenceRecord;

/// Read-only view of the evidence a run has actually captured.
///
/// Every method is fail-closed by construction: `None`/`false` means
/// "not proven", never "probably fine".
pub trait EvidenceView {
    /// Canonical, repository-relative identity of `path`, resolving
    /// `.`/`..` and symlinks so equivalent spellings collapse to one
    /// name. `None` when `path` resolves outside the project root (or
    /// cannot be resolved at all), which must block admission rather
    /// than fabricate a relative spelling.
    fn repo_relative_identity(&self, path: &Path) -> Option<String>;

    /// The most recent read evidence for `path` whose captured hash
    /// still matches the file on disk. `None` when nothing was read,
    /// the read is stale, or the path is not admissible.
    fn fresh_evidence(&self, path: &Path) -> Option<EvidenceRecord>;

    /// Whether `id` names a record this run actually captured, so a
    /// model cannot cite an invented evidence id.
    fn knows_evidence(&self, id: &str) -> bool;
}
