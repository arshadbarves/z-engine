//! `WorkOrder` and `ActiveWorkOrder`: typed task contracts that bind
//! agent goals to evidence-backed writable paths, acceptance commands,
//! and target symbols.  Validation is fail-closed: every writable path
//! must reference fresh evidence from the ledger.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::evidence::EvidenceLedger;

/// A shell command the agent must run (and pass) before the order is
/// considered complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCommand {
    pub command: String,
    pub description: String,
}

/// A typed task contract: what the agent is allowed to do, and the
/// evidence trail backing those permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    pub id: String,
    pub goal: String,
    pub writable_paths: Vec<PathBuf>,
    pub target_symbols: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub acceptance_commands: Vec<AcceptanceCommand>,
}

/// Errors from work-order validation (fail-closed).
#[derive(Debug, thiserror::Error)]
pub enum WorkOrderError {
    #[error("work order missing goal")]
    MissingGoal,
    #[error("work order has no writable paths")]
    NoWritablePaths,
    #[error("writable path {path} has no backing evidence in ledger")]
    UnbackedPath { path: PathBuf },
    #[error("evidence id {id} not found in ledger")]
    EvidenceNotFound { id: String },
    #[error("ledger read failed: {0}")]
    LedgerError(#[from] crate::evidence::EvidenceError),
}

impl WorkOrder {
    /// Validate that every evidence id exists in the ledger and every
    /// writable path is covered by at least one referenced evidence
    /// record. Returns `Err` if validation fails (fail-closed).
    pub fn validate(&self, ledger: &EvidenceLedger) -> Result<(), WorkOrderError> {
        if self.goal.is_empty() {
            return Err(WorkOrderError::MissingGoal);
        }
        if self.writable_paths.is_empty() {
            return Err(WorkOrderError::NoWritablePaths);
        }
        let all_records = ledger.read_all()?;
        let record_ids: HashSet<&str> = all_records.iter().map(|r| r.id.as_str()).collect();
        // Every referenced evidence id must exist in the ledger.
        for eid in &self.evidence_ids {
            if !record_ids.contains(eid.as_str()) {
                return Err(WorkOrderError::EvidenceNotFound { id: eid.clone() });
            }
        }
        // Build set of paths covered by the referenced evidence records.
        let covered_paths: HashSet<&str> = all_records
            .iter()
            .filter(|r| self.evidence_ids.contains(&r.id))
            .map(|r| r.path.as_str())
            .collect();
        // Every writable path must be covered.
        for wp in &self.writable_paths {
            let wp_str = wp.to_string_lossy();
            if !covered_paths.contains(wp_str.as_ref()) {
                return Err(WorkOrderError::UnbackedPath { path: wp.clone() });
            }
        }
        Ok(())
    }
}

/// The single active work order in the agent loop, set via
/// `set_work_order` and consumed by the prompt builder and mutation
/// gates.
#[derive(Debug, Clone)]
pub struct ActiveWorkOrder {
    pub order: WorkOrder,
}

impl ActiveWorkOrder {
    pub fn new(order: WorkOrder) -> Self {
        Self { order }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{BlobHandle, EvidenceLedger, EvidenceRecord};

    fn make_record(id: &str, path: &str) -> EvidenceRecord {
        let mut r = EvidenceRecord::new(
            path,
            None,
            "0".repeat(64),
            BlobHandle::of(b"test"),
            "read_file",
            "working-tree",
        );
        r.id = id.to_string();
        r
    }

    fn make_order(evidence_ids: Vec<&str>, paths: Vec<&str>) -> WorkOrder {
        WorkOrder {
            id: "wo-1".into(),
            goal: "fix the bug".into(),
            writable_paths: paths.into_iter().map(PathBuf::from).collect(),
            target_symbols: vec![],
            evidence_ids: evidence_ids.into_iter().map(String::from).collect(),
            acceptance_commands: vec![AcceptanceCommand {
                command: "cargo test".into(),
                description: "run tests".into(),
            }],
        }
    }

    #[test]
    fn validate_rejects_order_without_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        // Empty ledger, order references evidence that doesn't exist.
        let order = make_order(vec!["ev-1"], vec!["src/main.rs"]);
        assert!(order.validate(&ledger).is_err());
    }

    #[test]
    fn validate_rejects_unbacked_path() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        ledger.append(&make_record("ev-1", "src/lib.rs")).unwrap();
        // Path is "src/main.rs" but evidence only covers "src/lib.rs".
        let order = make_order(vec!["ev-1"], vec!["src/main.rs"]);
        let err = order.validate(&ledger).unwrap_err();
        assert!(matches!(err, WorkOrderError::UnbackedPath { .. }));
    }

    #[test]
    fn validate_accepts_fully_backed_order() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        ledger.append(&make_record("ev-1", "src/main.rs")).unwrap();
        let order = make_order(vec!["ev-1"], vec!["src/main.rs"]);
        assert!(order.validate(&ledger).is_ok());
    }

    #[test]
    fn validate_rejects_empty_goal() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        ledger.append(&make_record("ev-1", "src/main.rs")).unwrap();
        let mut order = make_order(vec!["ev-1"], vec!["src/main.rs"]);
        order.goal = String::new();
        assert!(matches!(
            order.validate(&ledger).unwrap_err(),
            WorkOrderError::MissingGoal
        ));
    }

    #[test]
    fn validate_rejects_no_writable_paths() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = EvidenceLedger::open(dir.path()).unwrap();
        let mut order = make_order(vec![], vec![]);
        order.writable_paths = vec![];
        assert!(matches!(
            order.validate(&ledger).unwrap_err(),
            WorkOrderError::NoWritablePaths
        ));
    }
}
