//! Who gets to say a guarded turn is finished.
//!
//! A model's final message is a *claim*. In a guarded run that changed
//! the workspace, the claim is worth nothing on its own: this module
//! re-checks the run against the order it declared —
//! [`crate::governance::VerificationRunner`] audits scope, compiles the
//! workspace, and runs the declared acceptance commands — and only a
//! complete, all-passing [`crate::governance::VerificationManifest`]
//! turns that claim into [`TurnOutcome::Completed`].
//!
//! Nothing here decides *what* counts as proof; that is the manifest's
//! verdict. This is the seam that gathers the run's facts, persists the
//! manifest beside the evidence that produced it, and reports the
//! refusal in the vocabulary the UI already speaks.
//!
//! Unguarded runs never reach past the first line: they complete on the
//! model's word exactly as they did before governance existed.

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use std::sync::Arc;
use std::sync::atomic::Ordering;

use z_engine_provider::ChatMessage;

use crate::governance::{Verdict, VerificationRunner, write_manifest};
use crate::tools::ToolCtx;

use super::events::{Command, Event};
use super::state::LoopState;
use super::stop_watch::{Stop, unwind, watch_for_abort};
use super::turn::TurnOutcome;

/// The gate this module speaks for, named in [`Event::TurnBlocked`].
const GATE: &str = "completion";

/// Decide whether the final answer just produced may end the turn.
///
/// `cmd_rx` is drained while the checks run: verification is the last
/// thing a turn does, so nothing else is listening for the user's abort,
/// and without this a stop request would wait out every check's timeout.
pub(super) async fn settle_completion(
    ctx: &ToolCtx,
    state: &mut LoopState,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
) -> TurnOutcome {
    // A run with nothing to account for — unguarded, or guarded and
    // genuinely still — has nothing to prove. A run that cannot say what
    // it did is a different thing entirely, and blocks.
    let plan = match ctx.verification_plan() {
        Ok(Some(plan)) => plan,
        Ok(None) => return TurnOutcome::Completed,
        Err(e) => {
            return TurnOutcome::Blocked {
                gate: GATE,
                reason: e.to_string(),
                manifest_path: None,
            };
        }
    };

    let _ = ev_tx.send(Event::StatusNote(format!(
        "verifying work order {} before completing the turn",
        plan.work_order_id
    )));
    let runner = VerificationRunner::new(&ctx.project_root).with_abort(Arc::clone(&ctx.abort));
    // Pinned rather than dropped on abort: dropping the future would kill
    // the child this task spawned but leave its process group unreaped,
    // so the stop is awaited — bounded — and what it stopped is recorded.
    let mut running = std::pin::pin!(runner.run(&plan));
    let (manifest, stop) = tokio::select! {
        manifest = &mut running => (manifest, None),
        stop = watch_for_abort(cmd_rx, &ctx.abort) => (unwind(running, &plan).await, Some(stop)),
    };

    // Persist first: the refusal points at the manifest, and a manifest
    // that could not be written is itself reported rather than ignored.
    let manifest_path = match state.run_dir.as_ref().map(|d| write_manifest(d, &manifest)) {
        Some(Ok(path)) => Some(path.display().to_string()),
        Some(Err(e)) => {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "verification manifest could not be written: {e}"
            )));
            None
        }
        None => None,
    };

    if let Some(stop) = stop {
        // The checks are no longer running, so the flag has done its job.
        // Leaving it set would make every later tool call in this run
        // believe the user is still asking to stop; a shutdown keeps it,
        // because there is no later turn to protect.
        if matches!(stop, Stop::Abort) {
            ctx.abort.store(false, Ordering::Relaxed);
        }
        let _ = ev_tx.send(Event::StatusNote(match &manifest_path {
            Some(path) => format!("verification stopped; what it reached is recorded in {path}"),
            None => "verification stopped before it finished".into(),
        }));
        return TurnOutcome::Aborted;
    }

    // The transcript keeps the evidence either way, so a follow-up turn
    // starts from what actually ran rather than from the model's belief.
    state.working.push(ChatMessage::user(format!(
        "[harness verification]\n{}",
        manifest.summary()
    )));

    match manifest.verdict() {
        Verdict::Complete => TurnOutcome::Completed,
        Verdict::Blocked(reason) => TurnOutcome::Blocked {
            gate: GATE,
            reason,
            manifest_path,
        },
    }
}

#[cfg(test)]
mod tests {
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
}
