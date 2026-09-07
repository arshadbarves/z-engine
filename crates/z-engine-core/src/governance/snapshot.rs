//! What the working tree actually looks like, twice: once as a guarded
//! run finds it, and once when that run claims to be done.
//!
//! Verification cannot take the mutation log's word for what changed —
//! the log only knows about writes the governed tools performed. A change
//! made by an approved shell command, an editor, a background process, or
//! anything else in the machine is invisible to it, including changes to
//! files the order *did* declare writable. Comparing two content
//! snapshots is what turns "these are the writes I authorized" into "this
//! is everything that happened".
//!
//! Two backends, one meaning. In a git repository the candidate set is
//! whatever git already knows differs from `HEAD` (plus every path the
//! baseline was watching), which is fast and honours `.gitignore` so
//! build output never looks like an edit. Everywhere else the tree is
//! walked directly. Either way a snapshot stores content hashes of the
//! bytes on disk, so the comparison is about content and never about
//! timestamps or git bookkeeping.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::evidence::BlobHandle;

use super::plan::{ChangeState, WorkspaceChange};

/// Directories never audited: version-control metadata, this harness's
/// own evidence, and conventional build output. A change inside them is
/// not a change to the project's source.
const EXCLUDED_DIRS: &[&str] = &[".git", ".hg", ".jj", ".z-engine", "target", "node_modules"];

/// Bounds on what a single snapshot will read, so a guarded run in an
/// enormous or pathological tree fails loudly instead of hanging.
const MAX_FILES: usize = 20_000;
const MAX_BYTES: u64 = 128 * 1024 * 1024;

/// Why a workspace could not be described. Every variant blocks: an
/// audit that cannot see the tree proves nothing about it.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error(
        "this workspace is too large to audit ({found} {unit}, limit {limit}); guarded mode needs \
         to compare the whole change set before it can trust one"
    )]
    TooLarge {
        found: u64,
        limit: u64,
        unit: &'static str,
    },
    #[error("could not read {path} while auditing the workspace: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not enumerate the workspace at {root}: {source}")]
    Walk {
        root: PathBuf,
        source: std::io::Error,
    },
}

/// Content hashes for the paths worth watching, keyed repository-relative.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    files: BTreeMap<PathBuf, ChangeState>,
}

impl WorkspaceSnapshot {
    /// Describe `root` now.
    ///
    /// `baseline`, when given, widens the candidate set with every path
    /// that snapshot was watching, so a file changed earlier and then
    /// deleted — or restored to its committed content — is still
    /// compared rather than quietly dropping out of view.
    pub fn capture(root: &Path, baseline: Option<&Self>) -> Result<Self, SnapshotError> {
        let mut candidates = match git_candidates(root) {
            Some(paths) => paths,
            None => walk_candidates(root)?,
        };
        if let Some(base) = baseline {
            candidates.extend(base.files.keys().cloned());
        }
        Self::hash_all(root, candidates)
    }

    /// Every path whose content differs between this snapshot (the
    /// baseline) and `now`.
    pub fn changes(&self, now: &Self) -> Vec<WorkspaceChange> {
        let mut out = Vec::new();
        for (path, state) in &now.files {
            if self.files.get(path) != Some(state) {
                out.push(WorkspaceChange {
                    path: path.clone(),
                    state: state.clone(),
                });
            }
        }
        // A path the later capture never considered is one this run made
        // disappear; `capture` keeps baseline paths as candidates, so this
        // is belt-and-braces rather than the common case.
        for path in self.files.keys() {
            if !now.files.contains_key(path) {
                out.push(WorkspaceChange {
                    path: path.clone(),
                    state: ChangeState::Missing,
                });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out.dedup_by(|a, b| a.path == b.path);
        out
    }

    fn hash_all(
        root: &Path,
        candidates: BTreeSet<PathBuf>,
    ) -> Result<WorkspaceSnapshot, SnapshotError> {
        if candidates.len() > MAX_FILES {
            return Err(SnapshotError::TooLarge {
                found: candidates.len() as u64,
                limit: MAX_FILES as u64,
                unit: "files",
            });
        }
        let mut files = BTreeMap::new();
        let mut total: u64 = 0;
        for rel in candidates {
            let absolute = root.join(&rel);
            let state = match std::fs::read(&absolute) {
                Ok(bytes) => {
                    total += bytes.len() as u64;
                    if total > MAX_BYTES {
                        return Err(SnapshotError::TooLarge {
                            found: total,
                            limit: MAX_BYTES,
                            unit: "bytes",
                        });
                    }
                    ChangeState::Present {
                        content_hash: BlobHandle::of(&bytes).to_string(),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => ChangeState::Missing,
                // A directory where a file used to be, or an unreadable
                // file, is a difference the audit must see rather than
                // guess about.
                Err(e) if e.kind() == std::io::ErrorKind::IsADirectory => continue,
                Err(source) => return Err(SnapshotError::Read { path: rel, source }),
            };
            files.insert(rel, state);
        }
        Ok(WorkspaceSnapshot { files })
    }

    #[cfg(test)]
    pub(super) fn watched(&self) -> Vec<PathBuf> {
        self.files.keys().cloned().collect()
    }
}

/// Paths git already knows differ from `HEAD`, relative to `root`.
/// `None` when this is not a git worktree or git is unavailable, which is
/// a reason to walk the tree rather than to trust nothing.
fn git_candidates(root: &Path) -> Option<BTreeSet<PathBuf>> {
    let top = run_git(root, &["rev-parse", "--show-toplevel"])?;
    // Both sides are canonicalized before they are compared: git reports
    // the real path, and a project root reached through a symlink (a
    // macOS temp directory, most commonly) would otherwise never match.
    let top = std::fs::canonicalize(top.trim()).ok()?;
    let base = std::fs::canonicalize(root).ok()?;
    let status = run_git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
        ],
    )?;
    let mut out = BTreeSet::new();
    for entry in status.split('\0') {
        // Each entry is `XY<space>path`; the two status columns are fixed
        // width, so the path starts at byte 3.
        if entry.len() < 4 {
            continue;
        }
        let absolute = top.join(&entry[3..]);
        let Ok(rel) = absolute.strip_prefix(&base) else {
            continue; // outside this project root: not ours to audit
        };
        if is_excluded(rel) {
            continue;
        }
        out.insert(rel.to_path_buf());
    }
    Some(out)
}

fn run_git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every file under `root`, excluding version-control metadata, this
/// harness's own evidence, and build output.
fn walk_candidates(root: &Path) -> Result<BTreeSet<PathBuf>, SnapshotError> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|source| SnapshotError::Walk {
            root: dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| SnapshotError::Walk {
                root: dir.clone(),
                source,
            })?;
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            if is_excluded(rel) {
                continue;
            }
            // Symlinks are recorded by the bytes they point at only when
            // they resolve to a regular file; a symlinked directory is
            // never descended, so a loop cannot hang the walk.
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(path),
                Ok(t) if t.is_file() => {
                    out.insert(rel.to_path_buf());
                    if out.len() > MAX_FILES {
                        return Err(SnapshotError::TooLarge {
                            found: out.len() as u64,
                            limit: MAX_FILES as u64,
                            unit: "files",
                        });
                    }
                }
                _ => continue,
            }
        }
    }
    Ok(out)
}

fn is_excluded(rel: &Path) -> bool {
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|name| EXCLUDED_DIRS.contains(&name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn changed(baseline: &WorkspaceSnapshot, now: &WorkspaceSnapshot) -> Vec<String> {
        baseline
            .changes(now)
            .into_iter()
            .map(|c| c.path.display().to_string())
            .collect()
    }

    #[test]
    fn an_edit_a_creation_and_a_deletion_are_all_changes() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "src/lib.rs", "fn a() {}\n");
        write(tmp.path(), "doomed.txt", "bye\n");
        let baseline = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();

        write(tmp.path(), "src/lib.rs", "fn a() -> u8 { 1 }\n");
        write(tmp.path(), "src/new.rs", "fn b() {}\n");
        std::fs::remove_file(tmp.path().join("doomed.txt")).unwrap();
        let now = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();

        assert_eq!(
            changed(&baseline, &now),
            ["doomed.txt", "src/lib.rs", "src/new.rs"]
        );
    }

    #[test]
    fn an_untouched_tree_shows_no_changes() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "src/lib.rs", "fn a() {}\n");
        let baseline = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();
        let now = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();
        assert!(baseline.changes(&now).is_empty());
    }

    /// A file rewritten with its original bytes changed nothing, and must
    /// not be reported as if it had.
    #[test]
    fn a_file_restored_to_its_original_bytes_is_not_a_change() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "f.txt", "one\n");
        let baseline = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();
        write(tmp.path(), "f.txt", "two\n");
        write(tmp.path(), "f.txt", "one\n");
        let now = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();
        assert!(baseline.changes(&now).is_empty());
    }

    /// Build output and this run's own evidence are not the agent's
    /// edits, and auditing them would refuse every honest run.
    #[test]
    fn build_output_and_harness_evidence_are_not_watched() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "src/lib.rs", "fn a() {}\n");
        write(tmp.path(), "target/debug/binary", "elf\n");
        write(tmp.path(), ".z-engine/runs/01ABC/ledger.jsonl", "{}\n");
        write(tmp.path(), "node_modules/pkg/index.js", "module\n");

        let snapshot = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();
        assert_eq!(snapshot.watched(), [PathBuf::from("src/lib.rs")]);
    }

    /// In a git worktree the candidate set comes from git, so ignored
    /// build output stays invisible while a real edit is still seen.
    #[test]
    fn a_git_worktree_reports_the_same_changes_git_status_would() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "t"],
        ] {
            assert!(
                Command::new("git")
                    .args(&args)
                    .current_dir(root)
                    .output()
                    .is_ok_and(|o| o.status.success()),
                "git {args:?} must succeed"
            );
        }
        write(root, "src/lib.rs", "fn a() {}\n");
        write(root, ".gitignore", "ignored/\n");
        write(root, "ignored/artifact", "junk\n");
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--quiet", "-m", "base"])
            .current_dir(root)
            .output()
            .unwrap();

        let baseline = WorkspaceSnapshot::capture(root, None).unwrap();
        assert!(
            baseline.watched().is_empty(),
            "a clean tree watches nothing"
        );

        write(root, "src/lib.rs", "fn a() -> u8 { 1 }\n");
        write(root, "ignored/artifact", "rebuilt\n");
        let now = WorkspaceSnapshot::capture(root, Some(&baseline)).unwrap();

        assert_eq!(changed(&baseline, &now), ["src/lib.rs"]);
    }
}
