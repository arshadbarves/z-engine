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

use tokio::sync::mpsc::UnboundedSender;

use z_engine_provider::ChatMessage;

use crate::governance::{Verdict, VerificationRunner, write_manifest};
use crate::tools::ToolCtx;

use super::events::Event;
use super::state::LoopState;
use super::turn::TurnOutcome;

/// The gate this module speaks for, named in [`Event::TurnBlocked`].
const GATE: &str = "completion";

/// Decide whether the final answer just produced may end the turn.
pub(super) async fn settle_completion(
    ctx: &ToolCtx,
    state: &mut LoopState,
    ev_tx: &UnboundedSender<Event>,
) -> TurnOutcome {
    // A run that changed nothing has nothing to prove, guarded or not.
    if !ctx.has_mutated() {
        return TurnOutcome::Completed;
    }
    // Guarded mutations cannot happen without an accepted order (the
    // mutation gate refuses them), so a missing plan here means the store
    // became unreadable. Fail closed rather than complete on a claim.
    let Some(plan) = ctx.verification_plan() else {
        return TurnOutcome::Blocked {
            gate: GATE,
            reason: "this run changed files but its work order is no longer readable, so nothing \
                     can be verified"
                .into(),
            manifest_path: None,
        };
    };

    let _ = ev_tx.send(Event::StatusNote(format!(
        "verifying work order {} before completing the turn",
        plan.work_order_id
    )));
    let manifest = VerificationRunner::new(&ctx.project_root).run(&plan).await;

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
        ctx.note_mutation(&repo.path().join("Cargo.toml"));
        (ctx, store, repo)
    }

    #[tokio::test]
    async fn an_unguarded_run_completes_on_the_model_s_word() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = plain_ctx(tmp.path());
        let mut st = state(None);
        assert!(matches!(
            settle_completion(&ctx, &mut st, &channel()).await,
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
            settle_completion(&ctx, &mut st, &channel()).await,
            TurnOutcome::Completed
        ));
    }

    #[tokio::test]
    async fn a_broken_change_blocks_at_the_completion_gate_and_writes_the_manifest() {
        let (ctx, _store, _repo) = mutated("[package\nname = \"fixture\"\n");
        let run_dir = tempfile::tempdir().unwrap();
        let mut st = state(Some(run_dir.path().to_path_buf()));

        let outcome = settle_completion(&ctx, &mut st, &channel()).await;
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

        let outcome = settle_completion(&ctx, &mut st, &channel()).await;
        assert!(matches!(outcome, TurnOutcome::Completed), "{outcome:?}");
        assert!(run_dir.path().join("verification.json").is_file());
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

        let outcome = settle_completion(&ctx, &mut st, &channel()).await;
        assert!(matches!(outcome, TurnOutcome::Completed), "{outcome:?}");
    }
}
