//! Completion-gate tests: what turns a model's claim into a finished
//! turn, and what a refused turn leaves behind for the next one.

use super::*;
use crate::governance::{AcceptanceCommand, WorkOrder};
use crate::tools::test_support::{guarded_ctx, plain_ctx};
use std::path::{Path, PathBuf};

const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";
const MANIFEST: &str = "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";

fn state(run_dir: Option<PathBuf>) -> LoopState {
    LoopState::for_test(run_dir)
}

fn order(paths: &[&str], evidence: &[&str], acceptance: &str) -> WorkOrder {
    WorkOrder {
        id: "wo-1".into(),
        goal: "describe the fixture".into(),
        writable_paths: paths.iter().map(PathBuf::from).collect(),
        target_symbols: vec![],
        evidence_ids: evidence.iter().map(|s| (*s).to_string()).collect(),
        acceptance_commands: vec![AcceptanceCommand {
            command: acceptance.into(),
            description: "acceptance".into(),
        }],
    }
}

fn cargo_fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), LIB).unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), MANIFEST).unwrap();
    tmp
}

fn channel() -> UnboundedSender<Event> {
    tokio::sync::mpsc::unbounded_channel().0
}

/// A command feed nobody sends on, so the abort watch never wins the
/// race and the verdict is the runner's alone.
fn commands() -> UnboundedReceiver<Command> {
    tokio::sync::mpsc::unbounded_channel().1
}

/// A guarded run set up over `Cargo.toml`, having just written
/// `content` to it.
fn mutated(content: &str) -> (ToolCtx, tempfile::TempDir, tempfile::TempDir) {
    let repo = cargo_fixture();
    let (ctx, store) = guarded_ctx(repo.path(), None);
    let bytes = std::fs::read(repo.path().join("Cargo.toml")).unwrap();
    let id = ctx
        .record_read_evidence(&ctx.resolve(Path::new("Cargo.toml")), None, &bytes, &bytes)
        .unwrap()
        .unwrap();
    ctx.set_work_order(&order(&["Cargo.toml"], &[&id], "cargo check"))
        .unwrap();
    std::fs::write(repo.path().join("Cargo.toml"), content).unwrap();
    ctx.note_mutation(&repo.path().join("Cargo.toml"), content.as_bytes());
    (ctx, store, repo)
}

#[tokio::test]
async fn an_unguarded_run_completes_on_the_model_s_word() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = plain_ctx(tmp.path());
    let mut st = state(None);
    assert!(matches!(
        settle_completion(&ctx, &mut st, &mut commands(), &channel()).await,
        TurnOutcome::Completed
    ));
    assert!(
        st.working.is_empty(),
        "no manifest belongs in an unguarded transcript"
    );
}

#[tokio::test]
async fn a_guarded_run_that_changed_nothing_still_completes() {
    let repo = cargo_fixture();
    let (ctx, _store) = guarded_ctx(repo.path(), None);
    let mut st = state(None);
    assert!(matches!(
        settle_completion(&ctx, &mut st, &mut commands(), &channel()).await,
        TurnOutcome::Completed
    ));
}

#[tokio::test]
async fn a_broken_change_blocks_at_the_completion_gate_and_writes_the_manifest() {
    let (ctx, _store, _repo) = mutated("[package\nname = \"fixture\"\n");
    let run_dir = tempfile::tempdir().unwrap();
    let mut st = state(Some(run_dir.path().to_path_buf()));

    let outcome = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    let TurnOutcome::Blocked {
        gate,
        reason,
        manifest_path,
    } = outcome
    else {
        panic!("a broken build must not complete: {outcome:?}");
    };
    assert_eq!(gate, "completion");
    assert!(reason.contains("cargo check"), "{reason}");
    let path = manifest_path.expect("the refusal must point at its evidence");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("workOrderId")
    );
    assert!(
        st.working.last().unwrap_or(&ChatMessage::user("")) != &ChatMessage::user(""),
        "the transcript keeps what actually ran"
    );
}

#[tokio::test]
async fn a_verified_change_completes() {
    let (ctx, _store, _repo) = mutated(&format!("{MANIFEST}description = \"fixture\"\n"));
    let run_dir = tempfile::tempdir().unwrap();
    let mut st = state(Some(run_dir.path().to_path_buf()));

    let outcome = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    assert!(matches!(outcome, TurnOutcome::Completed), "{outcome:?}");
    assert!(run_dir.path().join("verification.json").is_file());
}

/// Stopping verification must not brick the rest of the run: the flag
/// that stopped the checks is cleared once they have stopped, and what
/// they reached is still recorded.
#[tokio::test]
async fn an_aborted_verification_unwinds_records_itself_and_clears_the_stop_flag() {
    let (ctx, _store, _repo) = mutated(&format!("{MANIFEST}description = \"fixture\"\n"));
    let run_dir = tempfile::tempdir().unwrap();
    let mut st = state(Some(run_dir.path().to_path_buf()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(Command::Abort).unwrap();

    let outcome = settle_completion(&ctx, &mut st, &mut rx, &channel()).await;

    assert!(matches!(outcome, TurnOutcome::Aborted), "{outcome:?}");
    assert!(
        !ctx.aborted(),
        "a stopped verification must not leave every later tool call refusing to run"
    );
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run_dir.path().join("verification.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest["complete"],
        serde_json::json!(false),
        "an interrupted verification proves nothing: {manifest}"
    );
    assert!(
        st.working.is_empty(),
        "a stopped turn has no verdict to put in the transcript"
    );
}

/// A change nothing recorded is the case the mutation log cannot see:
/// it must block, and must say which path it could not account for.
#[tokio::test]
async fn a_change_no_tool_recorded_blocks_the_completion() {
    let (ctx, _store, repo) = mutated(&format!("{MANIFEST}description = \"fixture\"\n"));
    // Written the way an approved shell command or a background
    // process would: on disk, with nothing in the mutation log.
    std::fs::write(
        repo.path().join("src/lib.rs"),
        format!("{LIB}// snuck in\n"),
    )
    .unwrap();
    let run_dir = tempfile::tempdir().unwrap();
    let mut st = state(Some(run_dir.path().to_path_buf()));

    let outcome = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;

    let TurnOutcome::Blocked { reason, .. } = outcome else {
        panic!("an unrecorded change cannot complete: {outcome:?}");
    };
    assert!(reason.contains("src/lib.rs"), "{reason}");
}

/// The fail-closed half of the same rule: if the run cannot read its
/// own record of what it changed, it cannot certify that it changed
/// nothing.
#[tokio::test]
async fn an_unreadable_mutation_log_blocks_instead_of_completing() {
    let repo = cargo_fixture();
    let (ctx, _store) = guarded_ctx(repo.path(), None);
    ctx.work_orders.as_ref().unwrap().poison_for_test();
    let mut st = state(None);

    let outcome = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;

    let TurnOutcome::Blocked { gate, reason, .. } = outcome else {
        panic!("an unreadable log cannot complete: {outcome:?}");
    };
    assert_eq!(gate, "completion");
    assert!(reason.contains("unreadable"), "{reason}");
}

/// An unwritable manifest is a reporting failure, not a licence to
/// complete or to block a run that genuinely verified.
#[tokio::test]
async fn a_manifest_that_cannot_be_written_does_not_change_the_verdict() {
    let (ctx, _store, _repo) = mutated(&format!("{MANIFEST}description = \"fixture\"\n"));
    let blocker = tempfile::tempdir().unwrap();
    let path = blocker.path().join("not-a-dir");
    std::fs::write(&path, b"x").unwrap();
    let mut st = state(Some(path));

    let outcome = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    assert!(matches!(outcome, TurnOutcome::Completed), "{outcome:?}");
}

/// The regression that made per-turn settlement necessary.
///
/// Turn 1 verifies, and verifying runs `cargo check`, which writes
/// `Cargo.lock`. If the run kept judging against the workspace it
/// started in, turn 2 would open owing an explanation for a file the
/// harness itself created, and a guarded session could complete exactly
/// once.
#[tokio::test]
async fn a_second_turn_is_not_charged_for_the_first_turn_s_verification() {
    let (ctx, _store, repo) = mutated(MANIFEST);
    let mut st = state(None);

    let first = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    assert!(
        matches!(first, TurnOutcome::Completed),
        "the fixture must verify for this test to mean anything: {first:?}"
    );
    assert!(
        repo.path().join("Cargo.lock").exists(),
        "cargo check must have written a lockfile, or there is nothing to regress on"
    );

    let second = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    assert!(
        matches!(second, TurnOutcome::Completed),
        "a turn that changed nothing must not answer for the last turn's checks: {second:?}"
    );
}

/// The other half of the same rule: settling must not become a way to
/// launder a change. A turn refused for an unaccounted edit still owes
/// that edit afterwards.
#[tokio::test]
async fn a_refused_turn_does_not_bless_the_change_it_was_refused_for() {
    let repo = cargo_fixture();
    let (ctx, _store) = guarded_ctx(repo.path(), None);
    let bytes = std::fs::read(repo.path().join("Cargo.toml")).unwrap();
    let id = ctx
        .record_read_evidence(&ctx.resolve(Path::new("Cargo.toml")), None, &bytes, &bytes)
        .unwrap()
        .unwrap();
    ctx.set_work_order(&order(&["Cargo.toml"], &[&id], "cargo check"))
        .unwrap();
    // Changed behind the harness's back: no governed tool recorded it.
    std::fs::write(repo.path().join("src/lib.rs"), "pub fn parse() {}\n").unwrap();

    let mut st = state(None);
    let first = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    assert!(
        matches!(first, TurnOutcome::Blocked { .. }),
        "an unrecorded change must block: {first:?}"
    );

    let second = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    let TurnOutcome::Blocked { reason, .. } = second else {
        panic!("being refused must not make the change acceptable: {second:?}");
    };
    assert!(
        reason.contains("src/lib.rs"),
        "the same unaccounted change must still be named: {reason}"
    );
}

/// A verified turn moves the line to the tree the checks produced, so a
/// change made *after* that turn is still visible as a change.
#[tokio::test]
async fn a_change_made_after_a_verified_turn_is_still_caught() {
    let (ctx, _store, repo) = mutated(MANIFEST);
    let mut st = state(None);
    let first = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    assert!(matches!(first, TurnOutcome::Completed), "{first:?}");

    std::fs::write(repo.path().join("src/lib.rs"), "pub fn parse() {}\n").unwrap();

    let second = settle_completion(&ctx, &mut st, &mut commands(), &channel()).await;
    let TurnOutcome::Blocked { reason, .. } = second else {
        panic!("a new unrecorded change must block: {second:?}");
    };
    assert!(reason.contains("src/lib.rs"), "{reason}");
}
