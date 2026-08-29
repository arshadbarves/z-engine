//! Guarded-mode wiring (opt-in, spec Task 4).
//!
//! A guarded run records every bounded read as durable evidence and works
//! under one declared work order. Both stores are created here, per run,
//! under `.z-engine/runs/<run-id>/`, and handed to the tool context; the
//! governance tool is registered only for these runs, so an unguarded run
//! keeps exactly the toolset and prompt it had before this feature.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;

use crate::evidence::{EvidenceError, EvidenceLedger, FsBlobStore};
use crate::governance::WorkOrderStore;
use crate::tools::{EvidenceStore, ToolRegistry, set_work_order::SetWorkOrderTool};

use super::LoopConfig;
use super::events::Event;

/// The per-run stores a guarded loop threads through `ToolCtx`.
pub(super) struct Guarded {
    pub(super) evidence: Arc<EvidenceStore>,
    pub(super) work_orders: Arc<WorkOrderStore>,
}

/// Prepare guarded mode when `cfg.guarded` is set: open this run's
/// evidence storage and register `set_work_order`.
///
/// Returns `None` for unguarded runs and, fail-closed, when storage
/// cannot be opened — without evidence there is nothing to ground a work
/// order in, so the governance tool stays unregistered and the UI is told
/// why rather than silently running as if guarded.
pub(super) fn attach(
    cfg: &LoopConfig,
    registry: &mut ToolRegistry,
    ev_tx: &UnboundedSender<Event>,
) -> Option<Guarded> {
    if !cfg.guarded {
        return None;
    }
    match open_run(&cfg.project_root) {
        Ok(evidence) => {
            registry.register(Arc::new(SetWorkOrderTool));
            let _ = ev_tx.send(Event::StatusNote(
                "guarded mode: reads are recorded as evidence; declare a work order before editing"
                    .into(),
            ));
            Some(Guarded {
                evidence: Arc::new(evidence),
                work_orders: Arc::new(WorkOrderStore::new()),
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "guarded mode unavailable");
            let _ = ev_tx.send(Event::StatusNote(format!("guarded mode unavailable: {e}")));
            None
        }
    }
}

/// Open `.z-engine/runs/<run-id>/` for this run's ledger and blobs.
fn open_run(project_root: &Path) -> Result<EvidenceStore, EvidenceError> {
    let dir = run_dir(project_root);
    let ledger = Arc::new(EvidenceLedger::open(&dir)?);
    let blobs = Arc::new(FsBlobStore::new(dir.join("blobs"))?);
    Ok(EvidenceStore::new(ledger, blobs))
}

fn run_dir(project_root: &Path) -> PathBuf {
    project_root
        .join(".z-engine")
        .join("runs")
        .join(ulid::Ulid::new().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(root: &Path, guarded: bool) -> LoopConfig {
        let mut cfg = LoopConfig::new("m", "http://127.0.0.1:1/v1");
        cfg.project_root = root.to_path_buf();
        cfg.guarded = guarded;
        cfg
    }

    #[test]
    fn unguarded_runs_get_no_stores_and_no_governance_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();
        assert!(attach(&cfg(tmp.path(), false), &mut registry, &tx).is_none());
        assert!(!registry.names().iter().any(|n| n == "set_work_order"));
        assert!(!tmp.path().join(".z-engine").exists());
    }

    #[test]
    fn guarded_runs_open_run_storage_and_register_the_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();
        let guarded = attach(&cfg(tmp.path(), true), &mut registry, &tx).expect("guarded wiring");
        assert!(registry.names().iter().any(|n| n == "set_work_order"));
        assert!(guarded.work_orders.active().is_none());
        let runs = std::fs::read_dir(tmp.path().join(".z-engine/runs"))
            .unwrap()
            .count();
        assert_eq!(runs, 1);
    }
}
