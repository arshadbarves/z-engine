//! The evidence side of a tool call: [`EvidenceStore`], the per-run
//! record of what was read, and the [`ToolCtx`] methods that write to and
//! read from it.
//!
//! Split out of `context.rs` so that file stays about the capability
//! bundle a tool is handed, while this one is about the single question
//! guarded mode keeps asking: what did this run actually see, and does
//! the file still say it?

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::evidence::{BlobHandle, BlobStore, EvidenceLedger, EvidenceRecord};

use super::ToolCtx;
use super::path_identity::{canonical_in_root, canonicalize_root, to_repo_relative};

/// Bundles the ledger and blob store used to record and check freshness of
/// file-read evidence, plus an in-memory index of the latest record per
/// path so repeated freshness checks don't need to replay the whole
/// on-disk ledger. This wires storage handles from the `evidence` module
/// onto `ToolCtx`; it reuses that module's hashing/CAS logic rather than
/// duplicating it.
pub struct EvidenceStore {
    ledger: Arc<EvidenceLedger>,
    blobs: Arc<dyn BlobStore + Send + Sync>,
    latest: Mutex<HashMap<PathBuf, EvidenceRecord>>,
}

impl EvidenceStore {
    pub fn new(ledger: Arc<EvidenceLedger>, blobs: Arc<dyn BlobStore + Send + Sync>) -> Self {
        Self {
            ledger,
            blobs,
            latest: Mutex::new(HashMap::new()),
        }
    }

    /// Whether this run's ledger holds a record with `id`. A ledger that
    /// cannot be read reports `false`: an unreadable transcript must never
    /// make a cited evidence id look genuine.
    pub(super) fn knows(&self, id: &str) -> bool {
        self.ledger
            .read_all()
            .map(|records| records.iter().any(|r| r.id == id))
            .unwrap_or(false)
    }

    /// The latest record captured for an already-canonicalized path,
    /// regardless of whether it still matches disk. Freshness is the
    /// caller's judgement here — the mutation gate must distinguish
    /// "never read" from "read, then changed" to explain its refusal.
    pub(super) fn latest_for(&self, canonical: &Path) -> Option<EvidenceRecord> {
        self.latest.lock().ok()?.get(canonical).cloned()
    }

    /// The latest record per path this run read. Completion verification
    /// re-checks these hashes, so a change to a file the run observed but
    /// never declared writable cannot pass unnoticed.
    ///
    /// A poisoned lock is an error, not an empty set: "read nothing" and
    /// "cannot say what was read" would otherwise be the same answer, and
    /// one of them silently drops every witness from the audit.
    pub(super) fn witnesses(&self) -> Result<Vec<EvidenceRecord>, WitnessesUnavailable> {
        let latest = self.latest.lock().map_err(|_| WitnessesUnavailable)?;
        Ok(latest.values().cloned().collect())
    }

    /// Poison the witness map the way a panicking tool would, so tests can
    /// exercise the fail-closed path rather than assert it exists.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = self.latest.lock().unwrap();
            panic!("poisoning the witness map");
        }));
        std::panic::set_hook(hook);
    }
}

/// The witness map could not be read; see [`EvidenceStore::witnesses`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this run's record of what it read is unreadable")]
pub struct WitnessesUnavailable;

impl std::fmt::Debug for EvidenceStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceStore").finish_non_exhaustive()
    }
}

impl ToolCtx {
    /// Attach a revision-scoped evidence recorder (builder style). Once
    /// set, successful bounded reads become durable, edit-authorizing
    /// evidence (Task 3+); leaving it unset preserves prior behavior.
    pub fn with_evidence(mut self, store: Arc<EvidenceStore>) -> Self {
        self.evidence = Some(store);
        self
    }

    /// Record one successful bounded read as immutable evidence: the
    /// returned range's bytes are stored once in the content-addressed
    /// blob store, a durable [`EvidenceRecord`] is appended to the
    /// ledger, and the in-memory freshness index is updated so later
    /// [`ToolCtx::fresh_read_evidence`] calls avoid re-scanning the
    /// on-disk ledger.
    ///
    /// `resolved_path` is canonicalized (symlinks resolved, `.`/`..`
    /// normalized) before being used as the record's path and the
    /// freshness-index key, so equivalent spellings of the same in-root
    /// file (`./f.rs`, `sub/../f.rs`, a symlink into the repo, ...) always
    /// key to the same evidence identity — see [`ToolCtx::fresh_read_evidence`].
    /// Returns `Ok(None)` — recording nothing — both when no evidence
    /// recorder is attached (unguarded mode) *and* when `resolved_path`
    /// canonicalizes outside the project root: evidence's `path` field is
    /// documented as repository-relative, so an outside-root read must
    /// never be forced into a fabricated "relative" spelling or silently
    /// authorize edits outside the project.
    ///
    /// Storage failures (once past those two skip cases) are typed and
    /// fail closed rather than silently dropping evidence a later guarded
    /// gate might otherwise trust.
    ///
    /// Callers must only invoke this for genuinely successful, non-binary
    /// reads — binary or failed reads must never become edit-authorizing
    /// evidence. `full_file_bytes` and `range_bytes` must both derive from
    /// the *same* read of the file the caller already used to build the
    /// displayed output — never from a second, independent read — so a
    /// concurrent write can never make the evidence describe bytes the
    /// model never saw.
    pub fn record_read_evidence(
        &self,
        resolved_path: &Path,
        line_range: Option<(u32, u32)>,
        full_file_bytes: &[u8],
        range_bytes: &[u8],
    ) -> Result<Option<String>, super::ToolError> {
        let Some(store) = &self.evidence else {
            return Ok(None);
        };
        let Some(canonical) = canonical_in_root(resolved_path, &self.project_root) else {
            return Ok(None); // outside-root reads are never recorded as evidence
        };
        let rel_path = to_repo_relative(&canonical, &canonicalize_root(&self.project_root));
        let file_hash = BlobHandle::of(full_file_bytes).to_string();
        let blob = store
            .blobs
            .put(range_bytes)
            .map_err(|e| super::ToolError::Failed(format!("recording read evidence: {e}")))?;
        let record = EvidenceRecord::new(
            rel_path,
            line_range,
            file_hash,
            blob,
            "read_file",
            git_head_or_working_tree(&self.project_root),
        );
        store
            .ledger
            .append(&record)
            .map_err(|e| super::ToolError::Failed(format!("recording read evidence: {e}")))?;
        let id = record.id.clone();
        if let Ok(mut latest) = store.latest.lock() {
            latest.insert(canonical, record);
        }
        Ok(Some(id))
    }

    /// The most recent read evidence for `path` (resolved and
    /// canonicalized the same way as [`ToolCtx::record_read_evidence`], so
    /// `./f.rs`, `sub/../f.rs`, and a symlink to `f.rs` all look up the
    /// same record), only if the file's content on disk still matches the
    /// hash captured at read time.
    ///
    /// `None` means no evidence recorder is attached, `path` canonicalizes
    /// outside the project root, nothing was ever read, or the file has
    /// since changed on disk — any of which must block edit-authorizing
    /// use of stale, foreign, or absent evidence.
    pub fn fresh_read_evidence(&self, path: &Path) -> Option<EvidenceRecord> {
        let store = self.evidence.as_ref()?;
        let resolved = self.resolve(path);
        let canonical = canonical_in_root(&resolved, &self.project_root)?;
        let record = {
            let latest = store.latest.lock().ok()?;
            latest.get(&canonical)?.clone()
        };
        let current = std::fs::read(&canonical).ok()?;
        let current_hash = BlobHandle::of(&current).to_string();
        (current_hash == record.file_hash).then_some(record)
    }
}

/// Best-effort git HEAD for `root`; falls back to `"working-tree"` when the
/// directory isn't a git repository or the command fails for any reason —
/// evidence capture must never depend on `git` being present or working.
fn git_head_or_working_tree(root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "working-tree".to_string())
}
