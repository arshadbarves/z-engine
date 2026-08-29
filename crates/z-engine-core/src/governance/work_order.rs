//! `WorkOrder`: the typed task contract a guarded agent must declare
//! before it is allowed to change anything — what the goal is, which
//! paths may be written, which symbols are targeted, which evidence
//! backs the claim, and how completion will be proven.
//!
//! Admission is fail-closed and *fresh*: a path is only writable when
//! this run captured a read of it whose bytes still match disk, and the
//! order cites that record. Old ledger entries prove nothing about the
//! file the model is about to edit. Path identity and freshness come
//! from [`EvidenceView`] (implemented over `ToolCtx`), never from
//! hashing or path normalization re-implemented here.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::active::ActiveWorkOrder;
use super::evidence_view::EvidenceView;

/// A command that must run and pass before the order can be called done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCommand {
    pub command: String,
    pub description: String,
}

/// A declared unit of guarded work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    /// Stable identifier the model chooses (or the harness defaults).
    pub id: String,
    /// One-line statement of what the change must achieve.
    pub goal: String,
    /// Paths this order may write. Stored repository-relative and
    /// canonical once admitted.
    pub writable_paths: Vec<PathBuf>,
    /// Symbols the change is expected to touch (Task 5 uses these).
    pub target_symbols: Vec<String>,
    /// Evidence records cited as backing for the writable paths.
    pub evidence_ids: Vec<String>,
    /// Commands proving the order is complete (Task 6 runs these).
    pub acceptance_commands: Vec<AcceptanceCommand>,
}

/// Why a work order was refused. Messages go to the model verbatim, so
/// each one says how to fix the order.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkOrderError {
    #[error("work order needs a non-empty goal")]
    MissingGoal,
    #[error("work order needs at least one writable path")]
    NoWritablePaths,
    #[error("writable path {path} resolves outside the project root")]
    PathOutsideRoot { path: PathBuf },
    #[error("evidence id {id} was not captured in this run")]
    UnknownEvidence { id: String },
    #[error(
        "no fresh evidence for writable path {path} — call read_file on it \
         (again, if it changed on disk) and cite the new evidence id"
    )]
    StaleEvidence { path: PathBuf },
    #[error(
        "writable path {path} is backed by evidence {expected}, which the \
         order does not cite — add it to evidence_ids"
    )]
    EvidenceNotCited { path: PathBuf, expected: String },
    #[error("work orders are unavailable: this run is not in guarded mode")]
    NotGuarded,
    #[error("work order store unavailable")]
    StoreUnavailable,
}

impl WorkOrder {
    /// Admit this order: check the goal, normalize every writable path to
    /// its canonical repository-relative identity, and require each one to
    /// be backed by cited, still-fresh read evidence.
    ///
    /// Returns the normalized [`ActiveWorkOrder`] (paths deduplicated,
    /// backing records attached) so callers cannot accidentally keep using
    /// the unnormalized input.
    pub fn validate(&self, view: &dyn EvidenceView) -> Result<ActiveWorkOrder, WorkOrderError> {
        if self.goal.trim().is_empty() {
            return Err(WorkOrderError::MissingGoal);
        }
        if self.writable_paths.is_empty() {
            return Err(WorkOrderError::NoWritablePaths);
        }
        let cited: HashSet<&str> = self.evidence_ids.iter().map(String::as_str).collect();
        for id in &cited {
            if !view.knows_evidence(id) {
                return Err(WorkOrderError::UnknownEvidence { id: (*id).into() });
            }
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut normalized = Vec::with_capacity(self.writable_paths.len());
        let mut backing = Vec::with_capacity(self.writable_paths.len());
        for path in &self.writable_paths {
            let identity = view
                .repo_relative_identity(path)
                .ok_or_else(|| WorkOrderError::PathOutsideRoot { path: path.clone() })?;
            let record = view
                .fresh_evidence(path)
                .ok_or_else(|| WorkOrderError::StaleEvidence { path: path.clone() })?;
            if !cited.contains(record.id.as_str()) {
                return Err(WorkOrderError::EvidenceNotCited {
                    path: path.clone(),
                    expected: record.id,
                });
            }
            // Equivalent spellings of one file collapse to a single entry;
            // first occurrence wins so the result stays deterministic.
            if seen.insert(identity.clone()) {
                normalized.push(PathBuf::from(identity));
                backing.push(record);
            }
        }

        Ok(ActiveWorkOrder::new(
            WorkOrder {
                writable_paths: normalized,
                ..self.clone()
            },
            backing,
        ))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::evidence::{BlobHandle, EvidenceRecord};
    use std::collections::HashMap;
    use std::path::Path;

    /// In-memory [`EvidenceView`] keyed by the *spelling* a test uses, so
    /// path normalization and freshness can be exercised without touching
    /// the filesystem. `identities` maps a spelling to its canonical
    /// repository-relative name (absent = outside the root); `fresh` maps a
    /// spelling to the record that still matches disk.
    #[derive(Default)]
    pub(in crate::governance) struct FakeView {
        pub identities: HashMap<String, String>,
        pub fresh: HashMap<String, EvidenceRecord>,
        pub known: HashSet<String>,
    }

    impl EvidenceView for FakeView {
        fn repo_relative_identity(&self, path: &Path) -> Option<String> {
            self.identities
                .get(&path.to_string_lossy().to_string())
                .cloned()
        }
        fn fresh_evidence(&self, path: &Path) -> Option<EvidenceRecord> {
            self.fresh.get(&path.to_string_lossy().to_string()).cloned()
        }
        fn knows_evidence(&self, id: &str) -> bool {
            self.known.contains(id)
        }
    }

    pub(in crate::governance) fn record(id: &str, path: &str) -> EvidenceRecord {
        let mut r = EvidenceRecord::new(
            path,
            Some((1, 3)),
            "0".repeat(64),
            BlobHandle::of(b"fn parse() {}"),
            "read_file",
            "working-tree",
        );
        r.id = id.to_string();
        r
    }

    /// A view where `spelling` canonicalizes to `identity` and has fresh
    /// evidence `id`.
    pub(in crate::governance) fn view_with(spelling: &str, identity: &str, id: &str) -> FakeView {
        let mut view = FakeView::default();
        view.identities.insert(spelling.into(), identity.into());
        view.fresh.insert(spelling.into(), record(id, identity));
        view.known.insert(id.into());
        view
    }

    pub(in crate::governance) fn order(paths: &[&str], evidence: &[&str]) -> WorkOrder {
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

    #[test]
    fn admits_order_backed_by_cited_fresh_evidence() {
        let view = view_with("./src/lib.rs", "src/lib.rs", "ev-1");
        let active = order(&["./src/lib.rs"], &["ev-1"]).validate(&view).unwrap();
        assert_eq!(active.order.writable_paths, [PathBuf::from("src/lib.rs")]);
        assert_eq!(active.evidence[0].id, "ev-1");
    }

    #[test]
    fn normalizes_equivalent_spellings_to_one_entry() {
        let mut view = view_with("./src/lib.rs", "src/lib.rs", "ev-1");
        view.identities
            .insert("src/../src/lib.rs".into(), "src/lib.rs".into());
        view.fresh
            .insert("src/../src/lib.rs".into(), record("ev-1", "src/lib.rs"));
        let active = order(&["./src/lib.rs", "src/../src/lib.rs"], &["ev-1"])
            .validate(&view)
            .unwrap();
        assert_eq!(active.order.writable_paths, [PathBuf::from("src/lib.rs")]);
        assert_eq!(active.evidence.len(), 1);
    }

    #[test]
    fn rejects_path_outside_the_project_root() {
        let mut view = view_with("src/lib.rs", "src/lib.rs", "ev-1");
        view.fresh
            .insert("/etc/passwd".into(), record("ev-1", "src/lib.rs"));
        let err = order(&["/etc/passwd"], &["ev-1"])
            .validate(&view)
            .unwrap_err();
        assert!(matches!(err, WorkOrderError::PathOutsideRoot { .. }));
    }

    #[test]
    fn rejects_writable_path_without_fresh_evidence() {
        let mut view = view_with("src/lib.rs", "src/lib.rs", "ev-1");
        // The path is in-root and the id is real, but the read went stale.
        view.fresh.clear();
        let err = order(&["src/lib.rs"], &["ev-1"])
            .validate(&view)
            .unwrap_err();
        assert!(matches!(err, WorkOrderError::StaleEvidence { .. }));
    }

    #[test]
    fn rejects_order_citing_evidence_this_run_never_captured() {
        let view = view_with("src/lib.rs", "src/lib.rs", "ev-1");
        let err = order(&["src/lib.rs"], &["ev-invented"])
            .validate(&view)
            .unwrap_err();
        assert!(matches!(err, WorkOrderError::UnknownEvidence { .. }));
    }

    #[test]
    fn rejects_path_whose_fresh_evidence_is_not_cited() {
        let mut view = view_with("src/lib.rs", "src/lib.rs", "ev-2");
        // `ev-1` exists but describes a different, older read.
        view.known.insert("ev-1".into());
        let err = order(&["src/lib.rs"], &["ev-1"])
            .validate(&view)
            .unwrap_err();
        assert!(matches!(
            err,
            WorkOrderError::EvidenceNotCited { expected, .. } if expected == "ev-2"
        ));
    }

    #[test]
    fn rejects_blank_goal_and_empty_path_list() {
        let view = view_with("src/lib.rs", "src/lib.rs", "ev-1");
        let mut blank = order(&["src/lib.rs"], &["ev-1"]);
        blank.goal = "   ".into();
        assert_eq!(
            blank.validate(&view).unwrap_err(),
            WorkOrderError::MissingGoal
        );
        assert_eq!(
            order(&[], &["ev-1"]).validate(&view).unwrap_err(),
            WorkOrderError::NoWritablePaths
        );
    }
}
