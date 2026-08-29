//! The facts a guarded completion is judged against, gathered from the
//! run that made them.
//!
//! [`crate::governance::VerificationRunner`] deliberately knows nothing
//! about tool internals, so this is where a run's own record — what it
//! changed, and what it read — is turned into a
//! [`VerificationPlan`]. Everything here delegates: canonical path
//! identity comes from `path_identity` (Task 3), the read witnesses come
//! straight off the evidence records, and the mutation log lives in the
//! guarded work-order store. Nothing is re-derived, and an unguarded run
//! has no store, so it produces no plan at all.

use std::path::Path;

use crate::governance::{ReadWitness, VerificationPlan};

use super::ToolCtx;
use super::path_identity::{canonical_in_root, canonicalize_root, to_repo_relative};

impl ToolCtx {
    /// Record that `resolved` was changed. Only guarded runs keep a log;
    /// an out-of-root path is not recorded because it has no
    /// repository-relative identity for the manifest to name.
    ///
    /// Call this *after* the bytes reach disk — a refused or failed edit
    /// must never make a run look like it changed something.
    pub fn note_mutation(&self, resolved: &Path) {
        let Some(store) = &self.work_orders else {
            return;
        };
        let Some(canonical) = canonical_in_root(resolved, &self.project_root) else {
            return;
        };
        let rel = to_repo_relative(&canonical, &canonicalize_root(&self.project_root));
        store.note_mutation(rel.into());
    }

    /// Whether this run has changed anything under governance.
    pub fn has_mutated(&self) -> bool {
        self.work_orders
            .as_ref()
            .is_some_and(|s| !s.mutated_paths().is_empty())
    }

    /// The plan for verifying this run's active work order, or `None`
    /// when the run is unguarded or has no accepted order to verify.
    pub fn verification_plan(&self) -> Option<VerificationPlan> {
        let store = self.work_orders.as_ref()?;
        let active = store.active()?;
        Some(VerificationPlan {
            work_order_id: active.order.id.clone(),
            goal: active.order.goal.clone(),
            scope: active.order.writable_paths.clone(),
            mutated: store.mutated_paths(),
            witnesses: self.read_witnesses(),
            acceptance: active.order.acceptance_commands.clone(),
        })
    }

    /// One witness per path this run read: the repository-relative path
    /// and the whole-file hash it had at read time.
    fn read_witnesses(&self) -> Vec<ReadWitness> {
        self.evidence
            .as_ref()
            .map(|store| {
                store
                    .witnesses()
                    .into_iter()
                    .map(|r| ReadWitness {
                        path: r.path.into(),
                        file_hash: r.file_hash,
                    })
                    .collect()
            })
            .unwrap_or_default()
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
        ctx.note_mutation(&tmp.path().join("f.rs"));
        assert!(!ctx.has_mutated());
        assert!(ctx.verification_plan().is_none());
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

        assert!(!ctx.has_mutated());
        // Equivalent spellings collapse to one canonical entry.
        ctx.note_mutation(&tmp.path().join("src/../src/lib.rs"));
        ctx.note_mutation(&tmp.path().join("src/lib.rs"));
        assert!(ctx.has_mutated());

        let plan = ctx.verification_plan().unwrap();
        assert_eq!(plan.work_order_id, "wo-1");
        assert_eq!(plan.scope, [PathBuf::from("src/lib.rs")]);
        assert_eq!(plan.mutated, [PathBuf::from("src/lib.rs")]);
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
        ctx.note_mutation(&outside.path().join("f.rs"));
        assert!(!ctx.has_mutated());
    }

    #[test]
    fn a_guarded_run_without_an_accepted_order_has_no_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        assert!(ctx.verification_plan().is_none());
    }
}
