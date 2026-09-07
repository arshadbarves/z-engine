//! The admitted order: [`ActiveWorkOrder`] (a [`WorkOrder`] that passed
//! validation, together with the evidence records backing it) and
//! [`WorkOrderStore`], the single-slot holder the tool writes and the
//! turn pipeline reads.
//!
//! Rendering lives here too: the digest is a structured restatement of
//! the order's own fields (like the repo map or the notes block), not
//! model instructions — the instructions that tell an agent *when* to
//! declare an order live in `prompts/system-main.md`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::evidence::EvidenceRecord;

use super::plan::MutationRecord;
use super::snapshot::WorkspaceSnapshot;
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

/// The mutation log could not be read.
///
/// Reported rather than swallowed: an empty log and an unreadable one
/// look identical to a caller that defaults, and one of them means "this
/// run changed nothing" while the other means "this run cannot say what
/// it changed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this run's mutation log is unreadable")]
pub struct MutationLogUnavailable;

/// Holds the one order a guarded run is currently working under, the
/// changes made under it, and the workspace as the run first found it.
///
/// Shared between the `set_work_order` tool (writer) and the turn
/// pipeline (reader); a poisoned lock reports no active order, which
/// keeps later gates fail-closed. The mutation log lives here rather than
/// on the tool context because only a guarded run has one: an unguarded
/// run has no store, so it records nothing and behaves exactly as it did
/// before governance existed.
#[derive(Debug, Default)]
pub struct WorkOrderStore {
    active: Mutex<Option<Arc<ActiveWorkOrder>>>,
    /// Path to the hash of the bytes the last authorized write left there.
    mutated: Mutex<BTreeMap<PathBuf, String>>,
    /// The tree as this run found it; `None` when nothing captured one,
    /// which verification treats as a reason to block a changed workspace
    /// rather than as an empty change set.
    baseline: Option<WorkspaceSnapshot>,
}

impl WorkOrderStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// The store a guarded run uses: it remembers the workspace it
    /// started from, so completion can compare against it rather than
    /// trusting the mutation log to be the whole story.
    pub fn with_baseline(baseline: WorkspaceSnapshot) -> Self {
        Self {
            baseline: Some(baseline),
            ..Self::default()
        }
    }

    /// The workspace as this run found it, if it was captured.
    pub fn baseline(&self) -> Option<&WorkspaceSnapshot> {
        self.baseline.as_ref()
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

    /// Record that `repo_relative` was changed under this run, and the
    /// hash of the bytes the tool left there. Called only after a
    /// mutation actually reached disk, so a refused edit never makes the
    /// run look like it changed something.
    ///
    /// A later write to the same path replaces the hash: what completion
    /// authorizes is the state the run finished in.
    pub fn note_mutation(&self, repo_relative: PathBuf, content_hash: String) {
        if let Ok(mut log) = self.mutated.lock() {
            log.insert(repo_relative, content_hash);
        }
    }

    /// Poison the mutation log the way a panicking tool would, so tests
    /// can exercise the fail-closed path rather than assert it exists.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = self.mutated.lock().unwrap();
            panic!("poisoning the mutation log");
        }));
        std::panic::set_hook(hook);
    }

    /// Every change this run authorized, ordered by path.
    ///
    /// Errors rather than defaulting: see [`MutationLogUnavailable`].
    pub fn mutations(&self) -> Result<Vec<MutationRecord>, MutationLogUnavailable> {
        let log = self.mutated.lock().map_err(|_| MutationLogUnavailable)?;
        Ok(log
            .iter()
            .map(|(path, content_hash)| MutationRecord {
                path: path.clone(),
                content_hash: content_hash.clone(),
            })
            .collect())
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

    #[test]
    fn the_mutation_log_keeps_one_final_hash_per_path_in_order() {
        let store = WorkOrderStore::new();
        assert!(store.mutations().unwrap().is_empty());
        store.note_mutation(PathBuf::from("src/lib.rs"), "hash-a".into());
        store.note_mutation(PathBuf::from("Cargo.toml"), "hash-b".into());
        store.note_mutation(PathBuf::from("src/lib.rs"), "hash-c".into());

        let logged: Vec<(PathBuf, String)> = store
            .mutations()
            .unwrap()
            .into_iter()
            .map(|m| (m.path, m.content_hash))
            .collect();
        assert_eq!(
            logged,
            [
                (PathBuf::from("Cargo.toml"), "hash-b".to_string()),
                // The last write wins: it is the state completion has to
                // account for, not the one before it.
                (PathBuf::from("src/lib.rs"), "hash-c".to_string()),
            ]
        );
    }

    /// A lock a panicking tool poisoned must not read as "changed
    /// nothing" — that is the one answer that would let an unverified run
    /// complete.
    #[test]
    fn a_poisoned_mutation_log_reports_an_error_rather_than_an_empty_log() {
        let store = WorkOrderStore::new();
        store.note_mutation(PathBuf::from("src/lib.rs"), "hash-a".into());
        store.poison_for_test();
        assert_eq!(store.mutations(), Err(MutationLogUnavailable));
    }

    #[test]
    fn a_store_without_a_baseline_says_so_rather_than_inventing_one() {
        assert!(WorkOrderStore::new().baseline().is_none());
        let tmp = tempfile::tempdir().unwrap();
        let snapshot = WorkspaceSnapshot::capture(tmp.path(), None).unwrap();
        assert!(WorkOrderStore::with_baseline(snapshot).baseline().is_some());
    }
}
