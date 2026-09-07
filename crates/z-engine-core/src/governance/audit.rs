//! Reconciling three accounts of a guarded run: what the harness
//! authorized, what the workspace shows, and what the run read.
//!
//! Kept apart from [`super::verify`] because it is the one part of
//! verification that is pure judgement over already-gathered facts plus a
//! re-read of the files in question — no processes, no timeouts, no
//! policy about commands. It is also the part most worth reading on its
//! own, because it is what makes the mutation log non-self-certifying:
//! the log says what the governed tools did, the snapshot says what
//! actually changed, and only their exact agreement is authorized.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::evidence::BlobHandle;

use super::manifest::ScopeBreach;
use super::plan::{ChangeState, VerificationPlan};

/// Everything the declared scope and the mutation log fail to account for.
///
/// Four questions, each of which can only be answered by looking outside
/// the log that claims the answer:
///
/// 1. did every authorized write stay inside the declared scope?
/// 2. does every authorized write still hold the bytes it authorized —
///    or did something else overwrite, revert, or delete it afterwards?
/// 3. is every change the workspace shows one of those authorized writes?
/// 4. does everything this run read outside its scope still say what it
///    said when it was read?
pub(super) fn audit(root: &Path, plan: &VerificationPlan) -> Vec<ScopeBreach> {
    let mut breaches = Vec::new();
    let authorized: BTreeSet<&PathBuf> = plan.mutated.iter().map(|m| &m.path).collect();

    for record in &plan.mutated {
        if !plan.scope.contains(&record.path) {
            breaches.push(ScopeBreach {
                path: record.path.clone(),
                reason: "changed but not declared writable".into(),
            });
        }
    }

    for path in &authorized {
        let expected = plan.authorized_hash(path).unwrap_or_default();
        let reason = match std::fs::read(root.join(path)) {
            Ok(bytes) if BlobHandle::of(&bytes).to_string() == expected => continue,
            Ok(_) => {
                "no longer holds the bytes this run wrote, so something outside this run changed \
                 it afterwards"
            }
            Err(_) => "was written by this run and is no longer readable where it was written",
        };
        breaches.push(ScopeBreach {
            path: (*path).clone(),
            reason: reason.into(),
        });
    }

    for change in &plan.changes {
        if authorized.contains(&change.path) {
            continue; // already judged against the hash it was authorized for
        }
        let reason = match change.state {
            ChangeState::Present { .. } => {
                "changed during this run, but no governed tool recorded the change"
            }
            ChangeState::Missing => {
                "disappeared during this run, but no governed tool recorded the change"
            }
        };
        breaches.push(ScopeBreach {
            path: change.path.clone(),
            reason: reason.into(),
        });
    }

    for witness in &plan.witnesses {
        if plan.scope.contains(&witness.path) || authorized.contains(&witness.path) {
            continue;
        }
        let reason = match std::fs::read(root.join(&witness.path)) {
            Ok(bytes) if BlobHandle::of(&bytes).to_string() == witness.file_hash => continue,
            Ok(_) => "changed since this run read it, outside the declared scope",
            Err(_) => "was read by this run and is now unreadable",
        };
        breaches.push(ScopeBreach {
            path: witness.path.clone(),
            reason: reason.into(),
        });
    }

    breaches.sort_by(|a, b| a.path.cmp(&b.path));
    breaches.dedup_by(|a, b| a.path == b.path);
    breaches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::plan::{MutationRecord, WorkspaceChange};

    fn hash(text: &str) -> String {
        BlobHandle::of(text.as_bytes()).to_string()
    }

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        PathBuf::from(rel)
    }

    fn plan(scope: &[&str], mutated: Vec<MutationRecord>) -> VerificationPlan {
        VerificationPlan {
            work_order_id: "wo".into(),
            goal: "g".into(),
            scope: scope.iter().map(PathBuf::from).collect(),
            mutated,
            changes: Vec::new(),
            witnesses: Vec::new(),
            acceptance: Vec::new(),
        }
    }

    #[test]
    fn an_authorized_write_that_still_holds_its_bytes_is_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "src/lib.rs", "fn a() {}\n");
        let mut p = plan(
            &["src/lib.rs"],
            vec![MutationRecord {
                path,
                content_hash: hash("fn a() {}\n"),
            }],
        );
        p.changes = vec![WorkspaceChange {
            path: PathBuf::from("src/lib.rs"),
            state: ChangeState::Present {
                content_hash: hash("fn a() {}\n"),
            },
        }];
        assert!(audit(tmp.path(), &p).is_empty());
    }

    /// The point of the whole module: a change nobody logged is a breach
    /// even though the mutation log is perfectly consistent with itself.
    #[test]
    fn a_change_no_tool_recorded_is_a_breach() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "src/lib.rs", "fn a() {}\n");
        write(tmp.path(), "sneaky.rs", "fn b() {}\n");
        let mut p = plan(
            &["src/lib.rs"],
            vec![MutationRecord {
                path: PathBuf::from("src/lib.rs"),
                content_hash: hash("fn a() {}\n"),
            }],
        );
        p.changes = vec![WorkspaceChange {
            path: PathBuf::from("sneaky.rs"),
            state: ChangeState::Present {
                content_hash: hash("fn b() {}\n"),
            },
        }];

        let breaches = audit(tmp.path(), &p);
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert_eq!(breaches[0].path, PathBuf::from("sneaky.rs"));
        assert!(breaches[0].reason.contains("no governed tool"));
    }

    /// Declaring a path writable buys the run one *recorded* write to it,
    /// not a licence for anything else to edit it.
    #[test]
    fn an_unlogged_change_inside_the_declared_scope_is_still_a_breach() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "src/lib.rs", "fn edited_by_someone_else() {}\n");
        let mut p = plan(&["src/lib.rs"], Vec::new());
        p.changes = vec![WorkspaceChange {
            path: PathBuf::from("src/lib.rs"),
            state: ChangeState::Present {
                content_hash: hash("fn edited_by_someone_else() {}\n"),
            },
        }];

        let breaches = audit(tmp.path(), &p);
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert_eq!(breaches[0].path, PathBuf::from("src/lib.rs"));
    }

    /// The log records what a tool wrote; if the bytes on disk are now
    /// different, the log is describing a file that no longer exists.
    #[test]
    fn an_authorized_write_overwritten_afterwards_is_a_breach() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "src/lib.rs", "fn agent_wrote_this() {}\n");
        let p = plan(
            &["src/lib.rs"],
            vec![MutationRecord {
                path,
                content_hash: hash("fn the_tool_reported_this_instead() {}\n"),
            }],
        );

        let breaches = audit(tmp.path(), &p);
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert!(
            breaches[0].reason.contains("outside this run"),
            "{breaches:?}"
        );
    }

    #[test]
    fn an_authorized_write_deleted_afterwards_is_a_breach() {
        let tmp = tempfile::tempdir().unwrap();
        let p = plan(
            &["gone.rs"],
            vec![MutationRecord {
                path: PathBuf::from("gone.rs"),
                content_hash: hash("fn a() {}\n"),
            }],
        );
        let breaches = audit(tmp.path(), &p);
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert!(breaches[0].reason.contains("no longer readable"));
    }

    #[test]
    fn a_write_outside_the_declared_scope_is_a_breach() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write(tmp.path(), "elsewhere.rs", "fn a() {}\n");
        let p = plan(
            &["src/lib.rs"],
            vec![MutationRecord {
                path,
                content_hash: hash("fn a() {}\n"),
            }],
        );
        let breaches = audit(tmp.path(), &p);
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert!(breaches[0].reason.contains("not declared writable"));
    }

    #[test]
    fn a_witness_that_changed_outside_the_scope_is_a_breach() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "read.rs", "the new contents\n");
        let mut p = plan(&[], Vec::new());
        p.witnesses = vec![super::super::plan::ReadWitness {
            path: PathBuf::from("read.rs"),
            file_hash: hash("what it said when we read it\n"),
        }];
        let breaches = audit(tmp.path(), &p);
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert!(breaches[0].reason.contains("since this run read it"));
    }
}
