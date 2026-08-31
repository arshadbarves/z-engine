//! Chat-scoped review: list files this session mutated and build diffs
//! against checkpoint pre-images (Claude Code `/diff` turn semantics).

use std::path::{Path, PathBuf};

use super::checkpoint::CheckpointStore;
use super::fsutil::unified_diff;
use super::write_file::rel;

/// One agent-touched path with net change vs its session baseline.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionFileChange {
    pub path: String,
    /// `added` | `modified` | `deleted`
    pub status: String,
    pub added: u32,
    pub deleted: u32,
}

/// Files this chat mutated that still differ from their checkpoint baseline.
pub fn list_session_changes(store: &CheckpointStore, root: &Path) -> Vec<SessionFileChange> {
    let mut out = Vec::new();
    for (abs, original) in store.earliest_baselines() {
        let Some((status, old, new)) = net_change(&original, &abs) else {
            continue;
        };
        let (added, deleted) = count_line_edits(&old, &new);
        out.push(SessionFileChange {
            path: rel(&abs, root),
            status: status.to_string(),
            added,
            deleted,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Unified diff for one session-touched path (baseline → current disk).
/// Always emits a real unified diff with `@@` hunks (git-compatible).
pub fn session_diff_for(
    store: &CheckpointStore,
    root: &Path,
    rel_path: &str,
) -> Result<String, String> {
    if Path::new(rel_path).is_absolute() {
        return Err("path must be relative to the workspace".into());
    }
    let want = root.join(rel_path);
    let want_canon = canonicalize_best_effort(&want);
    let baselines = store.earliest_baselines();
    let Some((_, original)) = baselines
        .into_iter()
        .find(|(p, _)| canonicalize_best_effort(p) == want_canon || p == &want)
    else {
        return Err(format!("not touched in this chat: {rel_path}"));
    };

    let Some((_status, old, new)) = net_change(&original, &want) else {
        return Err(format!("not touched in this chat: {rel_path}"));
    };
    Ok(unified_diff(&old, &new, rel_path))
}

fn net_change(original: &Option<Vec<u8>>, abs: &Path) -> Option<(&'static str, String, String)> {
    let current = std::fs::read(abs).ok();
    match (original, current) {
        (None, None) => None,
        (None, Some(c)) => Some(("added", String::new(), lossy(&c))),
        (Some(o), None) => Some(("deleted", lossy(o), String::new())),
        (Some(o), Some(c)) if o == &c => None,
        (Some(o), Some(c)) => Some(("modified", lossy(o), lossy(&c))),
    }
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn count_line_edits(old: &str, new: &str) -> (u32, u32) {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut added = 0u32;
    let mut deleted = 0u32;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => deleted += 1,
            _ => {}
        }
    }
    (added, deleted)
}

fn canonicalize_best_effort(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_net_session_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let p = root.join("a.txt");
        std::fs::write(&p, "v0\n").unwrap();

        let store = CheckpointStore::default();
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "v1\n").unwrap();

        let list = list_session_changes(&store, root);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].path, "a.txt");
        assert_eq!(list[0].status, "modified");
        assert_eq!(list[0].added, 1);
        assert_eq!(list[0].deleted, 1);

        let diff = session_diff_for(&store, root, "a.txt").unwrap();
        assert!(diff.contains("@@"));
        assert!(diff.contains("-v0"));
        assert!(diff.contains("+v1"));
    }

    #[test]
    fn created_file_diff_has_hunks_and_content() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let p = root.join("new.txt");

        let store = CheckpointStore::default();
        store.begin_turn();
        store.snapshot_file(&p); // missing → created
        std::fs::write(&p, "hello\nworld\n").unwrap();

        let list = list_session_changes(&store, root);
        assert_eq!(list[0].status, "added");
        assert_eq!(list[0].added, 2);
        assert_eq!(list[0].deleted, 0);

        let diff = session_diff_for(&store, root, "new.txt").unwrap();
        assert!(
            diff.contains("@@"),
            "created-file diff must include hunk headers: {diff}"
        );
        assert!(diff.contains("+hello"));
        assert!(diff.contains("+world"));
    }

    #[test]
    fn deleted_file_counts_and_diffs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let p = root.join("gone.txt");
        std::fs::write(&p, "bye\n").unwrap();

        let store = CheckpointStore::default();
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::remove_file(&p).unwrap();

        let list = list_session_changes(&store, root);
        assert_eq!(list[0].status, "deleted");
        assert_eq!(list[0].added, 0);
        assert_eq!(list[0].deleted, 1);

        let diff = session_diff_for(&store, root, "gone.txt").unwrap();
        assert!(diff.contains("@@"));
        assert!(diff.contains("-bye"));
    }

    #[test]
    fn unchanged_after_revert_is_omitted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let p = root.join("b.txt");
        std::fs::write(&p, "same").unwrap();

        let store = CheckpointStore::default();
        store.begin_turn();
        store.snapshot_file(&p);
        let list = list_session_changes(&store, root);
        assert!(list.is_empty());
    }
}
