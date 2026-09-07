//! Audit tests: who is answerable for each difference between the three
//! accounts of a guarded run.

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
        workspace: WorkspaceSnapshot::default(),
        witnesses: Vec::new(),
        acceptance: Vec::new(),
    }
}

fn snapshot_of(root: &Path) -> WorkspaceSnapshot {
    WorkspaceSnapshot::capture(root, None).unwrap()
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

/// The lockfile cargo rewrites while answering a check is cargo's, not
/// the agent's: no later turn could account for it.
#[test]
fn the_lockfile_a_check_refreshes_is_the_harness_s_own() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "Cargo.lock", "before\n");
    let pre = snapshot_of(tmp.path());
    write(tmp.path(), "Cargo.lock", "after\n");
    let post = snapshot_of(tmp.path());

    let residue = reconcile_after_checks(&pre, &post);

    assert_eq!(residue.harness.len(), 1);
    assert_eq!(residue.harness[0].path, PathBuf::from("Cargo.lock"));
    assert!(residue.breaches.is_empty());
}

/// The reason this reconciliation exists: an acceptance command that
/// edits sources after the audit has passed must not slip through.
#[test]
fn a_source_edit_made_after_the_audit_is_a_breach() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "src/lib.rs", "fn a() {}\n");
    let pre = snapshot_of(tmp.path());
    write(tmp.path(), "src/lib.rs", "fn a() -> u8 { 1 }\n");
    let post = snapshot_of(tmp.path());

    let residue = reconcile_after_checks(&pre, &post);

    assert!(residue.harness.is_empty());
    assert_eq!(residue.breaches.len(), 1);
    assert_eq!(residue.breaches[0].path, PathBuf::from("src/lib.rs"));
    assert!(
        residue.breaches[0].reason.contains("after the audit"),
        "{}",
        residue.breaches[0].reason
    );
}

/// A quiet check window is the ordinary case and must stay silent.
#[test]
fn checks_that_touch_nothing_leave_no_residue() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "src/lib.rs", "fn a() {}\n");
    let pre = snapshot_of(tmp.path());
    let residue = reconcile_after_checks(&pre, &snapshot_of(tmp.path()));

    assert!(residue.harness.is_empty() && residue.breaches.is_empty());
}
