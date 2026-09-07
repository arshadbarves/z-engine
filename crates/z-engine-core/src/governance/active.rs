//! The admitted order: [`ActiveWorkOrder`] (a [`WorkOrder`] that passed
//! validation, together with the evidence records backing it) and
//! [`WorkOrderStore`], the single-slot holder the tool writes and the
//! turn pipeline reads.
//!
//! Rendering lives here too: the digest is a structured restatement of
//! the order's own fields (like the repo map or the notes block), not
//! model instructions — the instructions that tell an agent *when* to
//! declare an order live in `prompts/system-main.md`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::evidence::EvidenceRecord;

use super::plan::{MutationRecord, WorkspaceChange};
use super::snapshot::WorkspaceSnapshot;
use super::turn_record::{TurnRecord, TurnRecordUnavailable};
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

/// Holds the one order a guarded run is currently working under, and
/// the per-turn record of what has already been judged.
///
/// Shared between the `set_work_order` tool (writer) and the turn
/// pipeline (reader); a poisoned lock reports no active order, which
/// keeps later gates fail-closed. The turn record lives here rather than
/// on the tool context because only a guarded run has one: an unguarded
/// run has no store, so it records nothing and behaves exactly as it did
/// before governance existed.
#[derive(Debug, Default)]
pub struct WorkOrderStore {
    active: Mutex<Option<Arc<ActiveWorkOrder>>>,
    /// What this run has already been judged on. See [`TurnRecord`] for
    /// why the baseline and the mutation log move together.
    turn: TurnRecord,
}

impl WorkOrderStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// The store a guarded run uses: it remembers the workspace the run
    /// started from, so completion can compare against it rather than
    /// trusting the mutation log to be the whole story.
    pub fn with_baseline(baseline: WorkspaceSnapshot) -> Self {
        Self {
            active: Mutex::new(None),
            turn: TurnRecord::with_baseline(baseline),
        }
    }

    /// The workspace this turn is judged against, if one was captured.
    pub fn baseline(&self) -> Result<Option<WorkspaceSnapshot>, TurnRecordUnavailable> {
        self.turn.baseline()
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

    /// Record that `repo_relative` was changed under this turn, and the
    /// hash of the bytes the tool left there. Called only after a
    /// mutation actually reached disk, so a refused edit never makes the
    /// turn look like it changed something.
    pub fn note_mutation(&self, repo_relative: PathBuf, content_hash: String) {
        self.turn.note_mutation(repo_relative, content_hash);
    }

    /// Every change this turn authorized, ordered by path.
    ///
    /// Errors rather than defaulting: see [`TurnRecordUnavailable`].
    pub fn mutations(&self) -> Result<Vec<MutationRecord>, TurnRecordUnavailable> {
        self.turn.mutations()
    }

    /// The turn verified: what the checks left behind becomes what the
    /// next turn is judged against.
    pub fn settle_verified(&self, settled: WorkspaceSnapshot) -> Result<(), TurnRecordUnavailable> {
        self.turn.settle_verified(settled)
    }

    /// The turn did not verify: it keeps owing everything except the
    /// writes the harness's own checks made.
    pub fn settle_refused(
        &self,
        harness_writes: &[WorkspaceChange],
    ) -> Result<(), TurnRecordUnavailable> {
        self.turn.settle_refused(harness_writes)
    }

    /// Poison the turn record the way a panicking tool would, so tests
    /// can exercise the fail-closed path rather than assert it exists.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        self.turn.poison_for_test();
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

    /// The store is a façade over the turn record; these pin that the
    /// delegation is wired, not the record's own semantics (see
    /// `governance::turn_record::tests`).
    #[test]
    fn the_store_reports_what_the_turn_record_holds() {
        let store = WorkOrderStore::new();
        assert!(store.mutations().unwrap().is_empty());
        store.note_mutation(PathBuf::from("src/lib.rs"), "hash-a".into());
        assert_eq!(store.mutations().unwrap()[0].content_hash, "hash-a");

        store.settle_verified(WorkspaceSnapshot::default()).unwrap();
        assert!(store.mutations().unwrap().is_empty());
    }

    /// A lock a panicking tool poisoned must not read as "changed
    /// nothing" — that is the one answer that would let an unverified run
    /// complete.
    #[test]
    fn a_poisoned_turn_record_reports_an_error_rather_than_an_empty_log() {
        let store = WorkOrderStore::new();
        store.note_mutation(PathBuf::from("src/lib.rs"), "hash-a".into());
        store.poison_for_test();
        assert_eq!(store.mutations(), Err(TurnRecordUnavailable));
    }

    #[test]
    fn a_store_without_a_baseline_says_so_rather_than_inventing_one() {
        assert!(WorkOrderStore::new().baseline().unwrap().is_none());
        let tmp = tempfile::tempdir().unwrap();
        let snapshot = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();
        assert!(
            WorkOrderStore::with_baseline(snapshot)
                .baseline()
                .unwrap()
                .is_some()
        );
    }
}
