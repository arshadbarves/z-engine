//! Work-order capabilities on [`ToolCtx`]: the guarded-mode store hookup
//! and the [`EvidenceView`] implementation governance validates against.
//!
//! Kept beside `context.rs` rather than inside it because this is a
//! different reason to change (governance admission), and kept *out* of
//! `governance` because the canonical path identity and freshness rules
//! it delegates to are Task 3's, owned by `ToolCtx`. Nothing here
//! re-implements hashing or path normalization: every answer comes from
//! [`ToolCtx::fresh_read_evidence`] and `path_identity`.

use std::path::Path;
use std::sync::Arc;

use crate::evidence::EvidenceRecord;
use crate::governance::{ActiveWorkOrder, EvidenceView, WorkOrder, WorkOrderError, WorkOrderStore};

use super::ToolCtx;
use super::path_identity::{canonical_in_root, canonicalize_root, to_repo_relative};

impl ToolCtx {
    /// Attach the run's single-slot work-order store (builder style).
    /// Leaving it unset keeps the run unguarded: `set_work_order` refuses
    /// and no order digest is ever pinned into the prompt.
    pub fn with_work_orders(mut self, store: Arc<WorkOrderStore>) -> Self {
        self.work_orders = Some(store);
        self
    }

    /// Validate `order` against this run's evidence and make it the active
    /// order. Fails closed when the run is unguarded, when the store is
    /// unusable, or when any writable path lacks cited fresh evidence.
    pub fn set_work_order(
        &self,
        order: &WorkOrder,
    ) -> Result<Arc<ActiveWorkOrder>, WorkOrderError> {
        let store = self
            .work_orders
            .as_ref()
            .ok_or(WorkOrderError::NotGuarded)?;
        store.set(order.validate(self)?)
    }

    /// The order this run is currently working under, if any.
    pub fn active_work_order(&self) -> Option<Arc<ActiveWorkOrder>> {
        self.work_orders.as_ref()?.active()
    }
}

impl EvidenceView for ToolCtx {
    fn repo_relative_identity(&self, path: &Path) -> Option<String> {
        let resolved = self.resolve(path);
        let canonical = canonical_in_root(&resolved, &self.project_root)?;
        Some(to_repo_relative(
            &canonical,
            &canonicalize_root(&self.project_root),
        ))
    }

    fn fresh_evidence(&self, path: &Path) -> Option<EvidenceRecord> {
        self.fresh_read_evidence(path)
    }

    fn knows_evidence(&self, id: &str) -> bool {
        self.evidence.as_ref().is_some_and(|store| store.knows(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::AcceptanceCommand;
    use crate::tools::test_support::{guarded_ctx, plain_ctx};
    use std::path::PathBuf;

    fn order(paths: &[&str], evidence: &[&str]) -> WorkOrder {
        WorkOrder {
            id: "wo-1".into(),
            goal: "make parse fallible".into(),
            writable_paths: paths.iter().map(PathBuf::from).collect(),
            target_symbols: vec!["parse".into()],
            evidence_ids: evidence.iter().map(|s| (*s).to_string()).collect(),
            acceptance_commands: vec![AcceptanceCommand {
                command: "cargo test".into(),
                description: "unit tests".into(),
            }],
        }
    }

    /// Record a whole-file read of `name` the way `read_file` does.
    fn read(ctx: &ToolCtx, name: &str, bytes: &[u8]) -> String {
        ctx.record_read_evidence(&ctx.resolve(Path::new(name)), None, bytes, bytes)
            .unwrap()
            .expect("in-root read must be recorded")
    }

    #[test]
    fn identity_collapses_equivalent_spellings_and_rejects_outside_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), b"pub fn parse() {}\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);

        for spelling in ["src/lib.rs", "./src/lib.rs", "src/../src/lib.rs"] {
            assert_eq!(
                ctx.repo_relative_identity(Path::new(spelling)).as_deref(),
                Some("src/lib.rs"),
                "{spelling}"
            );
        }
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("f.txt"), b"x\n").unwrap();
        assert_eq!(
            ctx.repo_relative_identity(&outside.path().join("f.txt")),
            None
        );
    }

    #[test]
    fn accepts_order_backed_by_a_fresh_read_of_an_equivalent_spelling() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), b"pub fn parse() {}\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        let id = read(&ctx, "./src/lib.rs", b"pub fn parse() {}\n");

        let active = ctx
            .set_work_order(&order(&["src/../src/lib.rs"], &[&id]))
            .unwrap();
        assert_eq!(active.order.writable_paths, [PathBuf::from("src/lib.rs")]);
        assert_eq!(ctx.active_work_order().unwrap().order.id, "wo-1");
    }

    #[test]
    fn rejects_order_whose_evidence_went_stale_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"before\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        let id = read(&ctx, "f.rs", b"before\n");
        std::fs::write(tmp.path().join("f.rs"), b"after\n").unwrap();

        let err = ctx.set_work_order(&order(&["f.rs"], &[&id])).unwrap_err();
        assert!(matches!(err, WorkOrderError::StaleEvidence { .. }));
        assert!(ctx.active_work_order().is_none());
    }

    #[test]
    fn rejects_invented_evidence_ids() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"before\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path(), None);
        read(&ctx, "f.rs", b"before\n");

        let err = ctx
            .set_work_order(&order(&["f.rs"], &["01INVENTED"]))
            .unwrap_err();
        assert!(matches!(err, WorkOrderError::UnknownEvidence { .. }));
    }

    #[test]
    fn unguarded_contexts_refuse_work_orders() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = plain_ctx(tmp.path());
        assert_eq!(
            ctx.set_work_order(&order(&["f.rs"], &[])).unwrap_err(),
            WorkOrderError::NotGuarded
        );
        assert!(ctx.active_work_order().is_none());
    }
}
