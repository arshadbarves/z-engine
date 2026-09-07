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
//!
//! # What is *not* audited, and why that is bounded
//!
//! `target/`, `node_modules/`, `.git/`, `.z-engine/` and everything
//! `.gitignore` covers are excluded, because hashing a cargo target
//! directory on every turn would cost more than the run. That exclusion
//! would be a hole if anything could write there unaccounted, so the
//! guarded run closes it from three sides rather than claiming a full
//! audit it does not perform:
//!
//! 1. no shell command runs in a guarded run unless its write set is
//!    provably empty (`perms::read_only`);
//! 2. every governed write is logged and hash-checked against the bytes
//!    the tool left behind, wherever it landed (`governance::audit`);
//! 3. and the exclusion is *overridden* for an explicit, bounded set of
//!    paths — see [`WorkspaceSnapshot::capture_watching`]. Anything the
//!    active order declares writable, anything a governed tool wrote, and
//!    anything the run read is snapshotted even when it sits under an
//!    ignored directory, so a declared ignored path is compared like any
//!    other and a check that rewrites one is caught.
//!
//! What remains outside is genuinely outside: an ignored path that no
//! order declared, no tool wrote and no read witnessed. `cargo check`
//! rewriting `target/debug/**` is the intended case, and arbitrary
//! `build.rs` / proc-macro code executing during verification is the
//! accepted residual risk (docs/deviations.md row 12).

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
        Self::capture_watching(root, baseline, &BTreeSet::new())
    }

    /// [`WorkspaceSnapshot::capture`], plus an explicit set of paths that
    /// are watched **even when they are ignored or excluded**.
    ///
    /// This is the audited ignored subset. It is deliberately a list of
    /// concrete repository-relative paths the run already named — the
    /// order's scope, the mutation log, the read witnesses — and never a
    /// pattern or a directory: a broader rule would either cost a walk of
    /// `target/` or let an unnamed ignored path in through the side.
    ///
    /// Watching one costs a single `read`, so the set is bounded by what
    /// the run itself declared, and a path that does not exist is
    /// recorded as [`ChangeState::Missing`] exactly like any other.
    pub fn capture_watching(
        root: &Path,
        baseline: Option<&Self>,
        watched: &BTreeSet<PathBuf>,
    ) -> Result<Self, SnapshotError> {
        let mut candidates = match git_candidates(root) {
            Some(paths) => paths,
            None => walk_candidates(root)?,
        };
        if let Some(base) = baseline {
            candidates.extend(base.files.keys().cloned());
        }
        candidates.extend(watched.iter().filter(|p| p.is_relative()).cloned());
        Self::hash_all(root, candidates)
    }

    /// The paths this snapshot is watching, so a later capture keeps
    /// comparing them. A path only enters the set by being declared,
    /// written, read, or already watched — never by being discovered.
    pub fn watched_paths(&self) -> BTreeSet<PathBuf> {
        self.files.keys().cloned().collect()
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

    /// Fold `changes` into this snapshot, so what they describe stops
    /// counting as a difference.
    ///
    /// The one legitimate caller is the turn boundary: the harness's own
    /// verification processes rewrite the cargo lockfile, and the next
    /// turn must not be asked to account for a change no agent made.
    /// Nothing else may move the line — a change the agent is answerable
    /// for is absorbed only by *verifying* it.
    pub fn absorb(&mut self, changes: &[WorkspaceChange]) {
        for change in changes {
            // A deletion is recorded rather than dropped: a snapshot that
            // simply forgot the path would see the same deletion again on
            // the next capture, because a deleted path stays a candidate.
            self.files.insert(change.path.clone(), change.state.clone());
        }
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

    #[cfg(test)]
    pub(super) fn is_watching(&self, path: &str) -> bool {
        self.files.contains_key(Path::new(path))
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
mod tests;
