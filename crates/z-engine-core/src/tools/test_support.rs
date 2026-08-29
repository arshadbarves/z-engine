//! Shared fixtures for tool unit tests: one definition of "a guarded
//! `ToolCtx`" so gate, work-order, and editing tests all exercise the same
//! wiring instead of three drifting copies.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::evidence::{BlobStore, EvidenceLedger, FsBlobStore};
use crate::governance::WorkOrderStore;
use crate::lsp::LspHealth;
use crate::perms::PolicyEngine;

use super::{EvidenceStore, ToolCtx};

/// A guarded `ToolCtx` rooted at `root`: per-run evidence storage, a
/// work-order slot, and — when `health` is given — a stubbed Rust
/// semantic provider so tests never spawn rust-analyzer.
///
/// The returned `TempDir` must stay bound for the whole test: dropping it
/// deletes the ledger and blob files out from under the store.
pub(crate) fn guarded_ctx(root: &Path, health: Option<LspHealth>) -> (ToolCtx, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let ledger = Arc::new(EvidenceLedger::open(dir.path()).unwrap());
    let blobs: Arc<dyn BlobStore + Send + Sync> =
        Arc::new(FsBlobStore::new(dir.path().join("blobs")).unwrap());
    let mut ctx = ToolCtx::new(
        root.to_path_buf(),
        Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
        tempfile::tempdir().unwrap().keep(),
    )
    .with_evidence(Arc::new(EvidenceStore::new(ledger, blobs)))
    .with_work_orders(Arc::new(WorkOrderStore::new()));
    if let Some(health) = health {
        ctx.semantics = Some(Arc::new(super::semantics::StubSemantics(health)));
    }
    (ctx, dir)
}

/// An unguarded `ToolCtx` (no evidence recorder, no work-order slot) —
/// exactly what every run looked like before governance existed.
pub(crate) fn plain_ctx(root: &Path) -> ToolCtx {
    ToolCtx::new(
        root.to_path_buf(),
        Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
        tempfile::tempdir().unwrap().keep(),
    )
}
