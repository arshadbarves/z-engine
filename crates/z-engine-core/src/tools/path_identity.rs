//! Canonical path identity shared by every path-comparing decision in
//! `ToolCtx`: evidence recording, freshness lookups, and the outside-root
//! containment check. Kept as its own module (split out of `context.rs`
//! purely by responsibility, not speculatively) so the three call sites
//! can never drift apart and silently disagree on what "the same file"
//! means — e.g. `./f.rs`, `sub/../f.rs`, and a symlink into the repo must
//! all resolve to one identity, while paths outside the project root must
//! never be treated as if they had a repository-relative spelling.

use std::path::{Path, PathBuf};

/// Best-effort canonical form of `resolved` (an already root-anchored
/// path): resolves symlinks and normalizes `.`/`..` components via
/// [`std::fs::canonicalize`]. Existing paths canonicalize directly; a
/// not-yet-existing path (e.g. a target about to be created) canonicalizes
/// its parent and rejoins the file name, so callers can still anchor it.
/// Returns `None` only when neither the path nor an existing ancestor can
/// be resolved at all.
fn canonicalize_best_effort(resolved: &Path) -> Option<PathBuf> {
    if resolved.exists() {
        std::fs::canonicalize(resolved).ok()
    } else {
        resolved
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .map(|parent| parent.join(resolved.file_name().unwrap_or_default()))
    }
}

/// Canonical form of the project root, falling back to the root as given
/// when it can't be canonicalized (e.g. it doesn't exist yet in a test).
pub(super) fn canonicalize_root(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// Canonicalizes `resolved` and, only if the result still falls inside
/// `root`'s own canonical form, returns it. This is the single shared
/// identity used everywhere a path must compare equal regardless of how it
/// was spelled (`./f`, `sub/../f`, a symlink into the repo, ...): evidence
/// recording, freshness lookups, and the outside-root containment check
/// all go through this so they can never disagree with each other.
pub(super) fn canonical_in_root(resolved: &Path, root: &Path) -> Option<PathBuf> {
    let canonical = canonicalize_best_effort(resolved)?;
    let canonical_root = canonicalize_root(root);
    canonical.starts_with(&canonical_root).then_some(canonical)
}

/// Canonical, forward-slash-separated path relative to `canonical_root`.
/// Callers must pass already-canonicalized `canonical_path`/`canonical_root`
/// (see [`canonical_in_root`]) so the result is stable across equivalent
/// spellings of the same in-root file, not just a lexical `strip_prefix`.
pub(super) fn to_repo_relative(canonical_path: &Path, canonical_root: &Path) -> String {
    let rel = canonical_path
        .strip_prefix(canonical_root)
        .unwrap_or(canonical_path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{BlobStore, EvidenceLedger, FsBlobStore};
    use crate::perms::PolicyEngine;
    use crate::tools::context::{EvidenceStore, ToolCtx};
    use std::sync::{Arc, Mutex};

    /// A `ToolCtx` rooted at `root` with a fresh, temporary evidence
    /// recorder attached. The returned `TempDir` must stay bound for the
    /// whole test — dropping it early deletes the ledger/blob files out
    /// from under the store.
    fn ctx_with_evidence(root: &Path) -> (ToolCtx, tempfile::TempDir) {
        let evidence_dir = tempfile::tempdir().unwrap();
        let ledger = Arc::new(EvidenceLedger::open(evidence_dir.path()).unwrap());
        let blobs: Arc<dyn BlobStore + Send + Sync> =
            Arc::new(FsBlobStore::new(evidence_dir.path().join("blobs")).unwrap());
        let ctx = ToolCtx::new(
            root.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
        .with_evidence(Arc::new(EvidenceStore::new(ledger, blobs)));
        (ctx, evidence_dir)
    }

    #[test]
    fn dot_relative_and_plain_relative_paths_share_evidence_identity() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello\n").unwrap();
        let (ctx, _evidence_dir) = ctx_with_evidence(tmp.path());

        let via_dot = ctx.resolve(Path::new("./f.txt"));
        let id = ctx
            .record_read_evidence(&via_dot, None, b"hello\n", b"hello\n")
            .unwrap()
            .expect("in-root read must be recorded");

        let record = ctx
            .fresh_read_evidence(Path::new("f.txt"))
            .expect("plain-relative lookup must find the ./-recorded evidence");
        assert_eq!(record.id, id);
        assert_eq!(record.path, "f.txt");
    }

    #[test]
    fn parent_dir_backreference_resolves_to_same_evidence_identity() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello\n").unwrap();
        let (ctx, _evidence_dir) = ctx_with_evidence(tmp.path());

        let via_backref = ctx.resolve(Path::new("sub/../f.txt"));
        let id = ctx
            .record_read_evidence(&via_backref, None, b"hello\n", b"hello\n")
            .unwrap()
            .unwrap();

        let record = ctx.fresh_read_evidence(Path::new("f.txt")).unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.path, "f.txt");
    }

    #[test]
    #[cfg(unix)]
    fn symlinked_path_resolves_to_same_evidence_identity_as_real_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("real.txt"), b"hello\n").unwrap();
        std::os::unix::fs::symlink(tmp.path().join("real.txt"), tmp.path().join("link.txt"))
            .unwrap();
        let (ctx, _evidence_dir) = ctx_with_evidence(tmp.path());

        let via_symlink = ctx.resolve(Path::new("link.txt"));
        let id = ctx
            .record_read_evidence(&via_symlink, None, b"hello\n", b"hello\n")
            .unwrap()
            .unwrap();

        let record = ctx.fresh_read_evidence(Path::new("real.txt")).unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.path, "real.txt");
    }

    #[test]
    fn outside_root_reads_are_never_recorded_as_evidence() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("f.txt"), b"hello\n").unwrap();
        let (ctx, _evidence_dir) = ctx_with_evidence(root.path());

        let resolved = ctx.resolve(&outside.path().join("f.txt"));
        assert!(ctx.is_outside_root(&resolved));
        let recorded = ctx
            .record_read_evidence(&resolved, None, b"hello\n", b"hello\n")
            .unwrap();
        assert!(recorded.is_none());
    }
}
