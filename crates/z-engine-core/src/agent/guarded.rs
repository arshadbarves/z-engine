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
use crate::governance::{SnapshotError, WorkOrderStore, WorkspaceSnapshot};
use crate::tools::{EvidenceStore, ToolRegistry, set_work_order::SetWorkOrderTool};

use super::LoopConfig;
use super::events::Event;

/// The per-run stores a guarded loop threads through `ToolCtx`, plus the
/// directory they live in — completion verification writes its manifest
/// beside the evidence that produced it.
#[derive(Debug)]
pub(super) struct Guarded {
    pub(super) evidence: Arc<EvidenceStore>,
    pub(super) work_orders: Arc<WorkOrderStore>,
    pub(super) dir: PathBuf,
}

/// A guarded run that could not be governed.
///
/// Distinct from "unguarded": the user asked for governance and it is
/// unavailable, which is a refusal, not a mode.
#[derive(Debug, thiserror::Error)]
pub(super) enum GuardedUnavailable {
    #[error("guarded mode unavailable: {0}")]
    Storage(#[from] EvidenceError),
    /// Without a picture of the workspace as the run started, completion
    /// could only ever trust the mutation log about what changed.
    #[error("guarded mode unavailable: {0}")]
    Workspace(#[from] SnapshotError),
}

/// Prepare guarded mode when `cfg.guarded` is set: open this run's
/// evidence storage and register `set_work_order`.
///
/// Returns `Ok(None)` for unguarded runs — nothing to prepare, behavior
/// unchanged from before this feature existed.
///
/// Returns `Err` when a *guarded* run's storage cannot be opened. Without
/// evidence there is nothing to ground a work order, or the mutation gate,
/// in; continuing would leave the run believing it is guarded while
/// executing ungoverned. The caller must terminate the run: the governance
/// tool stays unregistered and the UI is told why.
pub(super) fn attach(
    cfg: &LoopConfig,
    registry: &mut ToolRegistry,
    ev_tx: &UnboundedSender<Event>,
) -> Result<Option<Guarded>, GuardedUnavailable> {
    if !cfg.guarded {
        return Ok(None);
    }
    let dir = run_dir(&cfg.project_root);
    match prepare(&cfg.project_root, &dir) {
        Ok((evidence, baseline)) => {
            registry.register(Arc::new(SetWorkOrderTool));
            prune_ungoverned(registry, ev_tx);
            let _ = ev_tx.send(Event::StatusNote(
                "guarded mode: reads are recorded as evidence; declare a work order before editing"
                    .into(),
            ));
            Ok(Some(Guarded {
                evidence: Arc::new(evidence),
                work_orders: Arc::new(WorkOrderStore::with_baseline(baseline)),
                dir,
            }))
        }
        Err(err) => {
            tracing::error!(error = %err, "refusing guarded run");
            let reason = format!("{err}; refusing to run ungoverned");
            // Error first so every consumer shows the detail, then the
            // terminal marker so none of them mistake the closing channel
            // for a run that finished normally.
            let _ = ev_tx.send(Event::Error(reason.clone()));
            let _ = ev_tx.send(Event::RunBlocked { reason });
            Err(err)
        }
    }
}

/// Everything a guarded run needs before its first tool call: durable
/// evidence storage, and the workspace as it stands right now.
fn prepare(
    project_root: &Path,
    dir: &Path,
) -> Result<(EvidenceStore, WorkspaceSnapshot), GuardedUnavailable> {
    let evidence = open_run(dir)?;
    let baseline = WorkspaceSnapshot::capture(project_root, None)?;
    Ok((evidence, baseline))
}

/// Remove every tool a guarded run has no governance for.
///
/// Guarded mode's promise is that nothing changes the workspace without
/// passing the mutation gate, and the gate only stands in front of tools
/// this crate owns. Anything registered from outside — an MCP server's
/// tools, most of all — can write files, run commands, and call network
/// services with no work order, no evidence, and no entry in the mutation
/// log, which would make the completion audit's change set unexplainable
/// even when the agent behaved. Pruning here rather than at each
/// registration site means a later registrar cannot forget the rule.
fn prune_ungoverned(registry: &mut ToolRegistry, ev_tx: &UnboundedSender<Event>) {
    let mut governed = ToolRegistry::guarded_builtins().names().to_vec();
    governed.sort();
    governed.dedup();
    let dropped = registry.retain(&governed);
    if !dropped.is_empty() {
        let _ = ev_tx.send(Event::StatusNote(format!(
            "guarded mode: {} unavailable ({} not governed by the mutation gate)",
            dropped.join(", "),
            match dropped.len() {
                1 => "it is",
                _ => "they are",
            }
        )));
    }
}

/// Open `dir` (`.z-engine/runs/<run-id>/`) for this run's ledger and blobs.
fn open_run(dir: &Path) -> Result<EvidenceStore, EvidenceError> {
    let ledger = Arc::new(EvidenceLedger::open(dir)?);
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
        assert!(
            attach(&cfg(tmp.path(), false), &mut registry, &tx)
                .unwrap()
                .is_none()
        );
        assert!(!registry.names().iter().any(|n| n == "set_work_order"));
        assert!(!tmp.path().join(".z-engine").exists());
    }

    #[test]
    fn guarded_runs_open_run_storage_and_register_the_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();
        let guarded = attach(&cfg(tmp.path(), true), &mut registry, &tx)
            .expect("storage must open")
            .expect("guarded wiring");
        assert!(registry.names().iter().any(|n| n == "set_work_order"));
        assert!(guarded.work_orders.active().is_none());
        assert!(
            guarded.dir.starts_with(tmp.path().join(".z-engine/runs")),
            "the manifest must land beside this run's evidence: {}",
            guarded.dir.display()
        );
        let runs = std::fs::read_dir(tmp.path().join(".z-engine/runs"))
            .unwrap()
            .count();
        assert_eq!(runs, 1);
    }

    /// Load-bearing for Task 5: if governance storage cannot be opened
    /// there is nothing to ground a work order — or a mutation gate — in.
    /// The run must be refused, never silently downgraded to an ungoverned
    /// one that still believes it is guarded.
    /// A tool registered from outside this crate — an MCP server's, in
    /// production — has no mutation gate in front of it, so a guarded run
    /// must not advertise it. Registered *before* `attach`, exactly as the
    /// agent task wires MCP servers today.
    #[test]
    fn guarded_runs_drop_tools_the_mutation_gate_does_not_govern() {
        struct Foreign;

        #[async_trait::async_trait]
        impl crate::tools::Tool for Foreign {
            fn name(&self) -> &str {
                "mcp__files__write"
            }
            fn description(&self) -> &str {
                "writes files with no work order, no evidence, no log"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            async fn run(
                &self,
                _input: serde_json::Value,
                _ctx: &crate::tools::ToolCtx,
            ) -> Result<crate::tools::ToolOutput, crate::tools::ToolError> {
                unreachable!("a guarded run must never be able to call this")
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();
        registry.register(Arc::new(Foreign));

        attach(&cfg(tmp.path(), true), &mut registry, &tx)
            .expect("storage must open")
            .expect("guarded wiring");

        assert!(
            !registry.names().iter().any(|n| n == "mcp__files__write"),
            "an ungoverned mutation tool cannot survive into a guarded run: {:?}",
            registry.names()
        );
        assert!(
            registry.names().iter().any(|n| n == "write_file"),
            "the governed built-ins must survive: {:?}",
            registry.names()
        );
        assert!(registry.names().iter().any(|n| n == "set_work_order"));
        let reported = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            reported
                .iter()
                .any(|e| matches!(e, Event::StatusNote(m) if m.contains("mcp__files__write"))),
            "the run has to say what it took away: {reported:?}"
        );
    }

    #[test]
    fn a_guarded_run_starts_from_a_baseline_of_the_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"fn a() {}\n").unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();

        let guarded = attach(&cfg(tmp.path(), true), &mut registry, &tx)
            .unwrap()
            .unwrap();

        assert!(
            guarded.work_orders.baseline().is_some(),
            "completion cannot audit a change set it has no baseline for"
        );
    }

    #[test]
    fn guarded_storage_failure_refuses_the_run_instead_of_degrading() {
        let tmp = tempfile::tempdir().unwrap();
        // A regular file where the run directory must go: unopenable.
        std::fs::create_dir_all(tmp.path().join(".z-engine")).unwrap();
        std::fs::write(tmp.path().join(".z-engine/runs"), b"not a directory").unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();

        let err = attach(&cfg(tmp.path(), true), &mut registry, &tx).unwrap_err();
        assert!(err.to_string().contains("guarded mode"), "{err}");
        assert!(
            !registry.names().iter().any(|n| n == "set_work_order"),
            "a refused guarded run must not advertise governance tools"
        );
        let reported = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            reported
                .iter()
                .any(|e| matches!(e, Event::Error(m) if m.contains("guarded mode"))),
            "{reported:?}"
        );
    }

    /// A refusal must be legible as a *blocked* run, not a clean exit: the
    /// detail arrives first, then the terminal marker, and nothing follows
    /// it before the channel closes.
    #[test]
    fn a_refused_run_ends_with_a_terminal_blocked_event() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".z-engine")).unwrap();
        std::fs::write(tmp.path().join(".z-engine/runs"), b"not a directory").unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();

        attach(&cfg(tmp.path(), true), &mut registry, &tx).unwrap_err();

        let reported = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
        let error_at = reported
            .iter()
            .position(|e| matches!(e, Event::Error(_)))
            .expect("the refusal explains itself");
        let blocked_at = reported
            .iter()
            .position(|e| matches!(e, Event::RunBlocked { .. }))
            .expect("the refusal is terminal");
        assert!(error_at < blocked_at, "{reported:?}");
        assert_eq!(blocked_at, reported.len() - 1, "{reported:?}");
        let Event::RunBlocked { reason } = &reported[blocked_at] else {
            unreachable!()
        };
        assert!(reason.contains("refusing to run ungoverned"), "{reason}");
    }

    #[test]
    fn a_blocked_run_is_distinguishable_on_the_wire() {
        let json = serde_json::to_value(Event::RunBlocked {
            reason: "guarded mode unavailable".into(),
        })
        .unwrap();
        assert_eq!(json["type"], "runBlocked");
        assert_eq!(json["reason"], "guarded mode unavailable");
    }
}
