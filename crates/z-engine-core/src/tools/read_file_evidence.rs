//! Evidence capture for `read_file` (spec Task 3): turns one successful
//! read's already-in-memory snapshot into a durable, revision-scoped
//! [`EvidenceRecord`] when a [`super::EvidenceStore`] is attached to the
//! [`ToolCtx`]. Kept separate from the `read_file` tool implementation
//! itself so the I/O and windowing logic there doesn't have to know how
//! evidence is shaped.

use std::path::Path;

use super::{ToolCtx, ToolError};

/// Record the just-read `full` snapshot as evidence, when an evidence
/// recorder is attached. `full` and the derived range must be the *exact*
/// bytes `read_file` already used to build the displayed output — this
/// function performs no I/O of its own and never re-reads `path`, so a
/// concurrent write to the file after `full` was captured can never change
/// what gets recorded. `line_range` is `None` for the whole-file (empty
/// file) case and `Some((first, last))` for a windowed text read.
///
/// Only reachable from the successful, non-binary path in
/// `ReadFileTool::run` — binary or failed reads never call this, so they
/// can never become edit-authorizing evidence.
pub(super) fn capture_read_evidence(
    ctx: &ToolCtx,
    path: &Path,
    full: &[u8],
    line_range: Option<(usize, usize)>,
) -> Result<Option<String>, ToolError> {
    if ctx.evidence.is_none() {
        return Ok(None);
    }
    let range = match line_range {
        Some((first, last)) => extract_line_range(full, first, last),
        None => full.to_vec(),
    };
    ctx.record_read_evidence(
        path,
        line_range.map(|(f, l)| (f as u32, l as u32)),
        full,
        &range,
    )
}

/// Raw bytes of the inclusive 1-based `[first, last]` line range, joined
/// with `\n` and without the display's line-number prefixes — the literal
/// content actually read, stored as this call's evidence blob.
fn extract_line_range(bytes: &[u8], first: usize, last: usize) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .lines()
        .skip(first.saturating_sub(1))
        .take(last + 1 - first)
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{BlobHandle, BlobStore, EvidenceLedger, FsBlobStore};
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    /// A `ToolCtx` rooted at `root` with a fresh evidence recorder
    /// attached, plus direct handles onto the ledger/blob store so tests
    /// can inspect exactly what was recorded (not just what
    /// `fresh_read_evidence` chooses to reveal after its own freshness
    /// filtering).
    fn ctx_with_evidence(
        root: &Path,
    ) -> (
        ToolCtx,
        Arc<dyn BlobStore + Send + Sync>,
        Arc<EvidenceLedger>,
    ) {
        let evidence_dir = tempfile::tempdir().unwrap().keep();
        let ledger = Arc::new(EvidenceLedger::open(&evidence_dir).unwrap());
        let blobs: Arc<dyn BlobStore + Send + Sync> =
            Arc::new(FsBlobStore::new(evidence_dir.join("blobs")).unwrap());
        let ctx = ToolCtx::new(
            root.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
        .with_evidence(Arc::new(super::super::EvidenceStore::new(
            Arc::clone(&ledger),
            Arc::clone(&blobs),
        )));
        (ctx, blobs, ledger)
    }

    #[test]
    fn extract_line_range_returns_only_requested_lines_without_prefixes() {
        let content = (1..=10).map(|i| format!("line{i}\n")).collect::<String>();
        let range = extract_line_range(content.as_bytes(), 4, 6);
        assert_eq!(String::from_utf8(range).unwrap(), "line4\nline5\nline6");
    }

    #[test]
    fn extract_line_range_single_line() {
        let content = "only one line\n";
        let range = extract_line_range(content.as_bytes(), 1, 1);
        assert_eq!(String::from_utf8(range).unwrap(), "only one line");
    }

    /// Regression guard for the single-snapshot fix. The original bug: the
    /// displayed body came from one read of the file, while evidence was
    /// captured from a *second*, independent read — so a concurrent write
    /// between the two could make the recorded evidence describe bytes the
    /// model never actually saw. This test is fully deterministic (no
    /// timing/races): `path` on disk holds content that provably differs
    /// from the in-memory `snapshot` handed to `capture_read_evidence`. If
    /// this function (or `ToolCtx::record_read_evidence`) ever regressed to
    /// re-reading `path` instead of using the given bytes, the recorded
    /// hash/blob would reflect *disk* content here, and every assertion
    /// below would fail every single time — not just occasionally, under
    /// real concurrent-write timing.
    #[test]
    fn capture_read_evidence_uses_the_given_snapshot_never_rereads_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let stale_disk_content: &[u8] = b"STALE DISK BYTES THE MODEL NEVER SAW\n";
        std::fs::write(tmp.path().join("ghost.txt"), stale_disk_content).unwrap();
        let (ctx, blobs, ledger) = ctx_with_evidence(tmp.path());

        // Stands in for "the exact bytes `read_file` already used to build
        // the displayed output" — deliberately different from what's on
        // disk right now.
        let snapshot = b"line1\nline2\nline3\n".to_vec();
        let id =
            capture_read_evidence(&ctx, &tmp.path().join("ghost.txt"), &snapshot, Some((1, 3)))
                .unwrap()
                .expect("evidence recorder is attached");

        let records = ledger.read_all().unwrap();
        let record = records.into_iter().find(|r| r.id == id).unwrap();

        // The recorded hash must match the snapshot's hash, never disk's.
        assert_eq!(record.file_hash, BlobHandle::of(&snapshot).to_string());
        assert_ne!(
            record.file_hash,
            BlobHandle::of(stale_disk_content).to_string()
        );

        // The stored range blob must hold the snapshot's range, not disk's.
        let range_bytes = blobs.get(&record.blob).unwrap();
        assert_eq!(range_bytes, b"line1\nline2\nline3".to_vec());

        // Consequence check: since disk content differs from the snapshot,
        // freshness must correctly report this evidence as stale against
        // the *current* file — confirming the hash really came from the
        // snapshot handed in, not from a fresh read of `ghost.txt` (which
        // would otherwise still match disk and look spuriously "fresh").
        assert!(ctx.fresh_read_evidence(Path::new("ghost.txt")).is_none());
    }
}
