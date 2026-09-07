//! The facts a guarded completion is judged against, gathered from the
//! run that made them.
//!
//! [`crate::governance::VerificationRunner`] deliberately knows nothing
//! about tool internals, so this is where a run's own record — what it
//! changed, what it read, and what the workspace shows — is turned into a
//! [`VerificationPlan`]. Everything here delegates: canonical path
//! identity comes from `path_identity` (Task 3), the read witnesses come
//! straight off the evidence records, the mutation log lives in the
//! guarded work-order store, and the change set comes from comparing the
//! workspace against the snapshot the run started from.
//!
//! [`ToolCtx::settle_turn`] is the other half: once a turn has been
//! judged, it decides what the *next* turn is judged against. A run is
//! not one turn, and a baseline frozen at the run's start would charge
//! every later turn for the checks the earlier ones ran.
//!
//! Nothing here defaults on failure. An unguarded run has no store and so
//! has no plan, which is a fact; a guarded run whose log or evidence
//! cannot be read has *no answer*, which is an error, because "I changed
//! nothing" and "I cannot tell you what I changed" must not look alike to
//! the gate that grants completion.

use std::path::Path;

use crate::evidence::BlobHandle;
use crate::governance::{
    PlanError, ReadWitness, Verification, VerificationPlan, WorkspaceSnapshot,
};

use super::ToolCtx;
use super::path_identity::{canonical_in_root, canonicalize_root, to_repo_relative};

impl ToolCtx {
    /// Record that `resolved` was changed, and the `written` bytes a tool
    /// left there. Only guarded runs keep a log; an out-of-root path is
    /// not recorded because it has no repository-relative identity for
    /// the manifest to name.
    ///
    /// Call this *after* the bytes reach disk — a refused or failed edit
    /// must never make a run look like it changed something — and pass
    /// the bytes the tool wrote rather than re-reading the file, so a
    /// racing writer cannot have its change authorized as ours.
    pub fn note_mutation(&self, resolved: &Path, written: &[u8]) {
        let Some(store) = &self.work_orders else {
            return;
        };
        let Some(canonical) = canonical_in_root(resolved, &self.project_root) else {
            return;
        };
        let rel = to_repo_relative(&canonical, &canonicalize_root(&self.project_root));
        store.note_mutation(rel.into(), BlobHandle::of(written).to_string());
    }

    /// The plan for verifying this run, or `Ok(None)` when there is
    /// genuinely nothing to verify: an unguarded run, or a guarded run
    /// that neither wrote anything nor sits in a changed workspace.
    ///
    /// `Err` means the run cannot describe itself, which blocks.
    pub fn verification_plan(&self) -> Result<Option<VerificationPlan>, PlanError> {
        let Some(store) = &self.work_orders else {
            return Ok(None); // unguarded: nothing was ever governed
        };
        let mutated = store
            .mutations()
            .map_err(|_| PlanError::TurnRecordUnavailable)?;
        let baseline = store
            .baseline()
            .map_err(|_| PlanError::TurnRecordUnavailable)?;
        // Captured whether or not there is a baseline to compare it to:
        // the checks are about to run, and the tree they are handed is
        // what their own writes have to be measured against afterwards.
        let workspace = WorkspaceSnapshot::capture(&self.project_root, baseline.as_ref())?;
        // A guarded run with no baseline cannot see third-party changes;
        // only a run that also changed nothing itself is safe to wave
        // through.
        let changes = baseline
            .as_ref()
            .map(|b| b.changes(&workspace))
            .unwrap_or_default();
        if mutated.is_empty() && changes.is_empty() {
            return Ok(None);
        }
        let active = store.active().ok_or(PlanError::NoActiveOrder)?;
        Ok(Some(VerificationPlan {
            work_order_id: active.order.id.clone(),
            goal: active.order.goal.clone(),
            scope: active.order.writable_paths.clone(),
            mutated,
            changes,
            workspace,
            witnesses: self.read_witnesses()?,
            acceptance: active.order.acceptance_commands.clone(),
        }))
    }

    /// Move the line this run is judged against, now that a turn has
    /// been judged.
    ///
    /// Called once per guarded turn, after the verdict and never before
    /// it. Two outcomes, deliberately asymmetric:
    ///
    /// - **verified** — the workspace the checks left behind becomes what
    ///   the next turn starts from, and the log of authorized writes is
    ///   cleared. This is the only way a change the agent made stops
    ///   needing to be accounted for, and it is reachable only from a
    ///   complete manifest.
    /// - **refused** — nothing the agent did is blessed. Only the
    ///   harness's own writes are absorbed, because the next turn cannot
    ///   be asked to explain a lockfile `cargo check` rewrote.
    ///
    /// A verification that could not re-read the workspace reports no
    /// settled state; that failure is itself a breach, so it cannot
    /// arrive here alongside a verified verdict.
    pub fn settle_turn(&self, verified: bool, outcome: &Verification) {
        let Some(store) = &self.work_orders else {
            return; // unguarded: nothing was ever governed
        };
        let outcome_of_settling = match (verified, outcome.settled.clone()) {
            (true, Some(settled)) => store.settle_verified(settled),
            _ => store.settle_refused(&outcome.harness_writes),
        };
        // An unreadable record leaves the line where it was, so the next
        // turn still owes everything this one did — which is the safe
        // direction, and the only one available from here.
        let _ = outcome_of_settling;
    }

    /// One witness per path this run read: the repository-relative path    /// and the whole-file hash it had at read time.
    fn read_witnesses(&self) -> Result<Vec<ReadWitness>, PlanError> {
        let Some(store) = self.evidence.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(store
            .witnesses()
            .map_err(|_| PlanError::WitnessesUnavailable)?
            .into_iter()
            .map(|r| ReadWitness {
                path: r.path.into(),
                file_hash: r.file_hash,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{AcceptanceCommand, WorkOrder};
    use crate::tools::test_support::{guarded_ctx, plain_ctx};
    use std::path::PathBuf;

    fn order(paths: &[&str], evidence: &[&str]) -> WorkOrder {
        WorkOrder {
            id: "wo-1".into(),
            goal: "make parse fallible".into(),
            writable_paths: paths.iter().map(PathBuf::from).collect(),
            target_symbols: vec![],
            evidence_ids: evidence.iter().map(|s| (*s).to_string()).collect(),
            acceptance_commands: vec![AcceptanceCommand {
                command: "cargo test".into(),
                description: "unit tests".into(),
            }],
        }
    }

    fn read(ctx: &ToolCtx, name: &str, bytes: &[u8]) -> String {
        ctx.record_read_evidence(&ctx.resolve(Path::new(name)), None, bytes, bytes)
            .unwrap()
            .expect("in-root read must be recorded")
    }

    #[test]
    fn unguarded_runs_record_nothing_and_produce_no_plan() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"x\n").unwrap();
        let ctx = plain_ctx(tmp.path());
        ctx.note_mutation(&tmp.path().join("f.rs"), b"x\n");
        assert!(ctx.verification_plan().unwrap().is_none());
    }

    #[test]
    fn a_guarded_plan_carries_the_order_scope_mutations_and_witnesses() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), b"pub fn parse() {}\n").unwrap();
        std::fs::write(tmp.path().join("notes.md"), b"# notes\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        let id = read(&ctx, "src/lib.rs", b"pub fn parse() {}\n");
        read(&ctx, "notes.md", b"# notes\n");
        ctx.set_work_order(&order(&["src/lib.rs"], &[&id])).unwrap();

        assert!(ctx.verification_plan().unwrap().is_none());
        // Equivalent spellings collapse to one canonical entry.
        ctx.note_mutation(&tmp.path().join("src/../src/lib.rs"), b"one\n");
        ctx.note_mutation(&tmp.path().join("src/lib.rs"), b"two\n");

        let plan = ctx.verification_plan().unwrap().unwrap();
        assert_eq!(plan.work_order_id, "wo-1");
        assert_eq!(plan.scope, [PathBuf::from("src/lib.rs")]);
        assert_eq!(plan.mutated_paths(), [PathBuf::from("src/lib.rs")]);
        assert_eq!(
            plan.mutated[0].content_hash,
            crate::evidence::BlobHandle::of(b"two\n").to_string(),
            "the last write is what completion has to account for"
        );
        assert_eq!(plan.acceptance.len(), 1);
        let mut witnessed: Vec<PathBuf> = plan.witnesses.iter().map(|w| w.path.clone()).collect();
        witnessed.sort();
        assert_eq!(
            witnessed,
            [PathBuf::from("notes.md"), PathBuf::from("src/lib.rs")]
        );
    }

    #[test]
    fn a_mutation_outside_the_project_root_is_not_logged() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("f.rs"), b"x\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        ctx.note_mutation(&outside.path().join("f.rs"), b"x\n");
        assert!(ctx.verification_plan().unwrap().is_none());
    }

    #[test]
    fn a_guarded_run_that_did_nothing_in_a_still_workspace_has_no_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        assert!(ctx.verification_plan().unwrap().is_none());
    }

    /// The whole point of snapshotting: a change nobody logged still has
    /// to be answered for, and with no order to answer with it blocks.
    #[test]
    fn a_change_no_tool_made_is_still_a_plan_to_answer_for() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        std::fs::write(tmp.path().join("snuck-in.rs"), b"fn x() {}\n").unwrap();

        let err = ctx
            .verification_plan()
            .expect_err("an unexplained change cannot verify as nothing to do");
        assert!(matches!(err, PlanError::NoActiveOrder), "{err:?}");
    }

    /// A poisoned mutation log must never read as "changed nothing".
    #[test]
    fn an_unreadable_mutation_log_blocks_rather_than_reporting_no_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        ctx.work_orders.as_ref().unwrap().poison_for_test();

        let err = ctx
            .verification_plan()
            .expect_err("an unreadable log cannot certify a clean run");
        assert!(matches!(err, PlanError::TurnRecordUnavailable), "{err:?}");
    }

    /// Same rule for the evidence side: no witnesses must not be
    /// indistinguishable from unreadable witnesses.
    #[test]
    fn unreadable_witnesses_block_rather_than_verifying_against_none() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"x\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        let id = read(&ctx, "f.rs", b"x\n");
        ctx.set_work_order(&order(&["f.rs"], &[&id])).unwrap();
        ctx.note_mutation(&tmp.path().join("f.rs"), b"x\n");
        ctx.evidence.as_ref().unwrap().poison_for_test();

        let err = ctx
            .verification_plan()
            .expect_err("unreadable witnesses cannot certify an empty audit");
        assert!(matches!(err, PlanError::WitnessesUnavailable), "{err:?}");
    }
}
