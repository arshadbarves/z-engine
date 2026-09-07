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

/// The bounded audited ignored subset (finding I7).
///
/// `target/` is excluded because hashing it every turn would cost more
/// than the run — but "excluded by default" must not mean "invisible".
/// A path the run explicitly named is watched wherever it lives, so a
/// change to it is a change like any other.
#[test]
fn an_explicitly_watched_path_is_audited_even_under_an_excluded_directory() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "target/generated.rs", "fn a() {}\n");
    write(tmp.path(), "target/noise.bin", "junk\n");

    let watched: BTreeSet<PathBuf> = [PathBuf::from("target/generated.rs")].into();
    let baseline = WorkspaceSnapshot::capture_watching(tmp.path(), None, &watched).unwrap();
    assert!(baseline.is_watching("target/generated.rs"));
    assert!(
        !baseline.is_watching("target/noise.bin"),
        "the subset stays bounded: only what the run named is watched"
    );

    write(tmp.path(), "target/generated.rs", "fn b() {}\n");
    write(tmp.path(), "target/noise.bin", "more junk\n");
    let now = WorkspaceSnapshot::capture_watching(tmp.path(), Some(&baseline), &watched).unwrap();
    assert_eq!(
        changed(&baseline, &now),
        ["target/generated.rs"],
        "a declared ignored path changes visibly; undeclared build output does not"
    );
}

/// …and once watched, it stays watched through the baseline, so a later
/// capture that is not told about it again still compares it.
#[test]
fn a_watched_ignored_path_survives_in_the_baseline() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "target/generated.rs", "fn a() {}\n");
    let watched: BTreeSet<PathBuf> = [PathBuf::from("target/generated.rs")].into();
    let baseline = WorkspaceSnapshot::capture_watching(tmp.path(), None, &watched).unwrap();
    assert_eq!(
        baseline.watched_paths(),
        watched,
        "the watched set is exactly what was asked for"
    );

    std::fs::remove_file(tmp.path().join("target/generated.rs")).unwrap();
    let now = WorkspaceSnapshot::capture(tmp.path(), Some(&baseline)).unwrap();
    assert_eq!(
        changed(&baseline, &now),
        ["target/generated.rs"],
        "a deletion under an excluded directory is still a change once watched"
    );
}

/// The subset is a list of paths, not a prefix: naming one file under
/// `target/` must not drag the rest of the directory into the audit.
#[test]
fn watching_a_path_never_widens_into_its_directory() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..5 {
        write(tmp.path(), &format!("target/debug/dep{i}.d"), "x\n");
    }
    write(tmp.path(), "target/kept.rs", "fn a() {}\n");
    let watched: BTreeSet<PathBuf> = [PathBuf::from("target/kept.rs")].into();
    let snapshot = WorkspaceSnapshot::capture_watching(tmp.path(), None, &watched).unwrap();
    assert_eq!(snapshot.watched(), [PathBuf::from("target/kept.rs")]);
}
