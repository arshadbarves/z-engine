//! Turn-record tests: the line only moves on a verified turn, and an
//! unreadable record refuses to answer rather than answering "nothing".

use super::*;

fn present(path: &str, hash: &str) -> WorkspaceChange {
    WorkspaceChange {
        path: PathBuf::from(path),
        state: ChangeState::Present {
            content_hash: hash.to_string(),
        },
    }
}

fn baseline_with(path: &str, hash: &str) -> WorkspaceSnapshot {
    let mut snapshot = WorkspaceSnapshot::default();
    snapshot.absorb(&[present(path, hash)]);
    snapshot
}

/// The ordinary path: a turn logs what it wrote, and verification can
/// read the log back.
#[test]
fn what_a_turn_wrote_is_what_the_log_reports() {
    let record = TurnRecord::default();
    record.note_mutation(PathBuf::from("src/b.rs"), "hash-b".into());
    record.note_mutation(PathBuf::from("src/a.rs"), "hash-a".into());

    let paths: Vec<_> = record
        .mutations()
        .unwrap()
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(
        paths,
        [PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
    );
}

/// Two writes to one path are one answer: the state the turn ended in.
#[test]
fn the_last_write_to_a_path_is_the_one_that_counts() {
    let record = TurnRecord::default();
    record.note_mutation(PathBuf::from("src/a.rs"), "first".into());
    record.note_mutation(PathBuf::from("src/a.rs"), "second".into());

    let logged = record.mutations().unwrap();
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0].content_hash, "second");
}

/// A verified turn hands the next one the workspace the checks left
/// behind, and starts it owing nothing.
#[test]
fn a_verified_turn_moves_the_line_and_clears_the_log() {
    let record = TurnRecord::with_baseline(baseline_with("src/a.rs", "before"));
    record.note_mutation(PathBuf::from("src/a.rs"), "after".into());

    record
        .settle_verified(baseline_with("src/a.rs", "after"))
        .unwrap();

    assert!(record.mutations().unwrap().is_empty());
    let carried = record.baseline().unwrap().unwrap();
    assert!(
        carried
            .changes(&baseline_with("src/a.rs", "after"))
            .is_empty(),
        "the next turn starts from the verified workspace"
    );
}

/// A refused turn keeps owing what it was refused for. Nothing about
/// being blocked makes an unaccounted change acceptable.
#[test]
fn a_refused_turn_still_owes_what_it_changed() {
    let record = TurnRecord::with_baseline(baseline_with("src/a.rs", "before"));
    record.note_mutation(PathBuf::from("src/a.rs"), "after".into());

    record.settle_refused(&[]).unwrap();

    assert_eq!(record.mutations().unwrap().len(), 1);
    let carried = record.baseline().unwrap().unwrap();
    assert_eq!(
        carried.changes(&baseline_with("src/a.rs", "after")).len(),
        1,
        "the change the turn was refused for is still a change"
    );
}

/// The exception, and the reason this method exists: the harness's own
/// checks write too, and a refused turn must not leave those writes for
/// the next turn to explain.
#[test]
fn a_refused_turn_absorbs_only_the_harness_s_own_writes() {
    let record = TurnRecord::with_baseline(baseline_with("Cargo.lock", "before"));
    record.note_mutation(PathBuf::from("src/a.rs"), "agent".into());

    record
        .settle_refused(&[present("Cargo.lock", "regenerated")])
        .unwrap();

    let carried = record.baseline().unwrap().unwrap();
    assert!(
        carried
            .changes(&baseline_with("Cargo.lock", "regenerated"))
            .is_empty(),
        "the lockfile the harness rewrote is not the agent's to explain"
    );
    assert_eq!(
        record.mutations().unwrap().len(),
        1,
        "and the agent's own writes are untouched"
    );
}

/// When the harness rewrites a file the turn had authorized, the log
/// follows the bytes that are actually on disk.
#[test]
fn a_harness_rewrite_of_an_authorized_path_updates_the_log() {
    let record = TurnRecord::with_baseline(baseline_with("Cargo.lock", "before"));
    record.note_mutation(PathBuf::from("Cargo.lock"), "by-agent".into());

    record
        .settle_refused(&[present("Cargo.lock", "by-cargo")])
        .unwrap();

    let logged = record.mutations().unwrap();
    assert_eq!(logged[0].content_hash, "by-cargo");
}

/// An unreadable record is not an empty one.
#[test]
fn an_unreadable_record_refuses_to_answer() {
    let record = TurnRecord::with_baseline(WorkspaceSnapshot::default());
    record.poison_for_test();

    assert!(record.mutations().is_err());
    assert!(record.baseline().is_err());
    assert!(
        record
            .settle_verified(WorkspaceSnapshot::default())
            .is_err()
    );
    assert!(
        record
            .settle_refused(&[present("Cargo.lock", "x")])
            .is_err()
    );
}
