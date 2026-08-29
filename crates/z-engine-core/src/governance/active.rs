//! The admitted order: [`ActiveWorkOrder`] (a [`WorkOrder`] that passed
//! validation, together with the evidence records backing it) and
//! [`WorkOrderStore`], the single-slot holder the tool writes and the
//! turn pipeline reads.
//!
//! Rendering lives here too: the digest is a structured restatement of
//! the order's own fields (like the repo map or the notes block), not
//! model instructions — the instructions that tell an agent *when* to
//! declare an order live in `prompts/system-main.md`.

use std::sync::{Arc, Mutex};

use crate::evidence::EvidenceRecord;

use super::work_order::{WorkOrder, WorkOrderError};

/// A validated order plus the fresh evidence that admitted it. Only
/// [`WorkOrder::validate`] can build one, so an `ActiveWorkOrder` always
/// carries canonical repository-relative writable paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWorkOrder {
    /// The normalized order (writable paths are repo-relative, deduped).
    pub order: WorkOrder,
    /// One fresh record per writable path, in the same order.
    pub evidence: Vec<EvidenceRecord>,
}

impl ActiveWorkOrder {
    pub(super) fn new(order: WorkOrder, evidence: Vec<EvidenceRecord>) -> Self {
        Self { order, evidence }
    }

    /// Deterministic restatement of the order for the prompt: same order
    /// in, same bytes out, no timestamps or counters.
    pub fn digest(&self) -> String {
        let mut out = String::from("# Active work order (guarded)\n");
        out.push_str(&format!("id: {}\n", self.order.id));
        out.push_str(&format!("goal: {}\n", self.order.goal));
        out.push_str("writable paths (nothing else may be changed):\n");
        for (path, record) in self.order.writable_paths.iter().zip(&self.evidence) {
            out.push_str(&format!(
                "- {} [evidence {} {}]\n",
                path.display(),
                record.id,
                range_label(record)
            ));
        }
        if !self.order.target_symbols.is_empty() {
            out.push_str("target symbols:\n");
            for symbol in &self.order.target_symbols {
                out.push_str(&format!("- {symbol}\n"));
            }
        }
        if !self.order.acceptance_commands.is_empty() {
            out.push_str("acceptance commands:\n");
            for cmd in &self.order.acceptance_commands {
                out.push_str(&format!("- `{}` — {}\n", cmd.command, cmd.description));
            }
        }
        out
    }

    /// One excerpt line per backing record, for the prompt's evidence
    /// section (see [`super::prompt::PromptSnapshot`]).
    pub fn evidence_excerpts(&self) -> Vec<String> {
        self.evidence
            .iter()
            .map(|r| format!("{} {} @{}", r.path, range_label(r), r.revision))
            .collect()
    }
}

/// Test-only assembly of an already-admitted order, so modules outside
/// `governance` can exercise rendering without standing up an evidence
/// store. Production code can only get here through [`WorkOrder::validate`].
#[cfg(test)]
impl ActiveWorkOrder {
    pub(crate) fn for_test(order: WorkOrder, evidence: Vec<EvidenceRecord>) -> Self {
        Self::new(order, evidence)
    }
}

fn range_label(record: &EvidenceRecord) -> String {
    match record.line_range {
        Some((first, last)) => format!("lines {first}-{last}"),
        None => "whole file".to_string(),
    }
}

/// Holds the one order a guarded run is currently working under.
/// Shared between the `set_work_order` tool (writer) and the turn
/// pipeline (reader); a poisoned lock reports no active order, which
/// keeps later gates fail-closed.
#[derive(Debug, Default)]
pub struct WorkOrderStore {
    active: Mutex<Option<Arc<ActiveWorkOrder>>>,
}

impl WorkOrderStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the active order (there is only ever one).
    pub fn set(&self, order: ActiveWorkOrder) -> Result<Arc<ActiveWorkOrder>, WorkOrderError> {
        let shared = Arc::new(order);
        let mut slot = self
            .active
            .lock()
            .map_err(|_| WorkOrderError::StoreUnavailable)?;
        *slot = Some(Arc::clone(&shared));
        Ok(shared)
    }

    pub fn active(&self) -> Option<Arc<ActiveWorkOrder>> {
        self.active.lock().ok()?.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::work_order::tests::{order, view_with};

    fn active() -> ActiveWorkOrder {
        let view = view_with("src/lib.rs", "src/lib.rs", "ev-1");
        order(&["src/lib.rs"], &["ev-1"]).validate(&view).unwrap()
    }

    #[test]
    fn digest_is_deterministic_and_states_scope() {
        let a = active();
        assert_eq!(a.digest(), a.digest());
        let digest = a.digest();
        assert!(digest.starts_with("# Active work order (guarded)\n"));
        assert!(digest.contains("goal: make parse fallible"));
        assert!(digest.contains("- src/lib.rs [evidence ev-1 lines 1-3]"));
        assert!(digest.contains("- parse"));
        assert!(digest.contains("- `cargo test` — unit tests"));
    }

    #[test]
    fn excerpts_name_the_backing_reads() {
        assert_eq!(
            active().evidence_excerpts(),
            ["src/lib.rs lines 1-3 @working-tree"]
        );
    }

    #[test]
    fn store_holds_exactly_one_order() {
        let store = WorkOrderStore::new();
        assert!(store.active().is_none());
        store.set(active()).unwrap();
        let mut second = active();
        second.order.goal = "second goal".into();
        store.set(second).unwrap();
        let held = store.active().unwrap();
        assert_eq!(held.order.goal, "second goal");
    }
}
