//! Snapshot tests: what counts as a change, what is deliberately not
//! watched, and what the two backends agree about.

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

/// Absorbing a change is how a settled turn stops asking about it: the
/// difference disappears, and only that difference.
#[test]
fn an_absorbed_change_stops_being_a_difference() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "Cargo.lock", "generated\n");
    write(tmp.path(), "src/lib.rs", "fn a() {}\n");
    let mut baseline = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();

    write(tmp.path(), "Cargo.lock", "regenerated\n");
    write(tmp.path(), "src/lib.rs", "fn a() -> u8 { 1 }\n");
    let now = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();

    let lockfile: Vec<WorkspaceChange> = baseline
        .changes(&now)
        .into_iter()
        .filter(|c| c.path == Path::new("Cargo.lock"))
        .collect();
    baseline.absorb(&lockfile);

    assert_eq!(
        changed(&baseline, &now),
        ["src/lib.rs"],
        "absorbing one change must not bless the others"
    );
}

/// A deletion has to be absorbed as a deletion; forgetting the path
/// would let the same missing file report itself again next time.
#[test]
fn an_absorbed_deletion_stays_absorbed() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "scratch.txt", "temporary\n");
    let mut baseline = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();
    std::fs::remove_file(tmp.path().join("scratch.txt")).unwrap();
    let now = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();

    baseline.absorb(&baseline.clone().changes(&now));

    let later = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();
    assert!(baseline.changes(&later).is_empty());
}
