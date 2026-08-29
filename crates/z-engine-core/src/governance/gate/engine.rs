//! The rules. Given [`MutationRequest`] facts, decide — in the order a
//! reviewer would ask — whether the change may touch the working tree.
//!
//! Two phases, because they cost different things. [`GateEngine::prescreen`]
//! answers everything a work order and the run's own evidence can settle:
//! no I/O, no language server, no waiting. Only when it passes is it worth
//! gathering Rust semantic facts, which [`GateEngine::localize`] then
//! judges. A caller holding every fact can use [`GateEngine::authorize`]
//! and get exactly the same verdict.

use std::path::{Path, PathBuf};

use crate::governance::ActiveWorkOrder;

use super::facts::{EvidenceState, LineRange, MutationRequest, RustFacts};
use super::failure::GateFailure;
use super::localize;

/// The gate's verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    /// Authorized: every claim the change makes is backed.
    Pass,
    /// Blocked, and reading (again) is what clears it.
    NeedsEvidence(GateFailure),
    /// Blocked for a reason more reading cannot fix.
    Fail(GateFailure),
}

impl GateDecision {
    /// Collapse to a typed result for adapters that only need
    /// pass/refuse; which kind of block it was survives in the failure.
    pub fn into_result(self) -> Result<(), GateFailure> {
        match self {
            Self::Pass => Ok(()),
            Self::NeedsEvidence(f) | Self::Fail(f) => Err(f),
        }
    }

    /// True when nothing is left to refuse — used by adapters that gather
    /// expensive facts only for requests still in the running.
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}

/// The pure decision procedure. No I/O, no clocks, no globals.
#[derive(Debug)]
pub struct GateEngine;

impl GateEngine {
    /// Everything decidable without a semantic provider: a declared work
    /// order, an in-root and in-scope path, and read evidence covering the
    /// lines about to change. Rust targets additionally need the order to
    /// *name* a symbol — a question no language server has to answer.
    pub fn prescreen(req: &MutationRequest<'_>) -> GateDecision {
        let Some(order) = req.order else {
            return GateDecision::Fail(GateFailure::NoWorkOrder);
        };
        let Some(identity) = req.identity else {
            return GateDecision::Fail(GateFailure::OutsideRoot {
                path: req.path.to_path_buf(),
            });
        };
        let scoped = Path::new(identity);
        if !order.order.writable_paths.iter().any(|p| p == scoped) {
            return GateDecision::Fail(GateFailure::OutOfScope {
                path: scoped.to_path_buf(),
                allowed: join_paths(&order.order.writable_paths),
            });
        }
        if req.rust && order.order.target_symbols.is_empty() {
            return GateDecision::Fail(GateFailure::NoTargetSymbol);
        }
        match evidence_gap(&req.evidence, req.changed, scoped) {
            Some(blocked) => GateDecision::NeedsEvidence(blocked),
            None => GateDecision::Pass,
        }
    }

    /// The Rust half: does the semantic provider place a declared target
    /// symbol in this file? See [`super::localize`] for why tree-sitter
    /// evidence cannot stand in for it.
    pub fn localize(facts: &RustFacts, order: &ActiveWorkOrder, path: &Path) -> GateDecision {
        localize::localize(facts, order, path)
    }

    /// Both phases at once. `rust` must be supplied whenever
    /// [`MutationRequest::rust`] is set: a Rust change with no gathered
    /// semantic facts is unproven, and unproven is refused.
    pub fn authorize(req: &MutationRequest<'_>, rust: Option<&RustFacts>) -> GateDecision {
        let prescreen = Self::prescreen(req);
        if !prescreen.is_pass() || !req.rust {
            return prescreen;
        }
        // Both are `Some`, or prescreen would not have passed.
        let (Some(order), Some(identity)) = (req.order, req.identity) else {
            return prescreen;
        };
        match rust {
            Some(facts) => Self::localize(facts, order, Path::new(identity)),
            None => GateDecision::Fail(GateFailure::SemanticEvidenceUnavailable {
                path: PathBuf::from(identity),
                reason: "no semantic facts were gathered for this change".into(),
            }),
        }
    }
}

/// The evidence half of the decision: read at all, still current, and
/// covering every line the change touches.
fn evidence_gap(
    evidence: &EvidenceState,
    changed: Option<LineRange>,
    path: &Path,
) -> Option<GateFailure> {
    match evidence {
        EvidenceState::Missing => Some(GateFailure::NoEvidence {
            path: path.to_path_buf(),
        }),
        EvidenceState::Stale => Some(GateFailure::StaleEvidence {
            path: path.to_path_buf(),
        }),
        EvidenceState::Fresh { covered, .. } if !covers(*covered, changed) => {
            Some(GateFailure::RangeNotCovered {
                path: path.to_path_buf(),
                changed: label(changed),
                covered: label(*covered),
            })
        }
        EvidenceState::Fresh { .. } => None,
    }
}

/// Does the evidence's `covered` span contain everything `changed`
/// touches? Whole-file evidence covers anything; a bounded read never
/// covers a whole-file rewrite.
fn covers(covered: Option<LineRange>, changed: Option<LineRange>) -> bool {
    match (covered, changed) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some((cf, cl)), Some((f, l))) => cf <= f && l <= cl,
    }
}

fn label(range: Option<LineRange>) -> String {
    match range {
        Some((first, last)) => format!("{first}-{last}"),
        None => "the whole file".to_string(),
    }
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::governance::gate::facts::{SemanticEvidence, SemanticHealth};
    use crate::governance::work_order::AcceptanceCommand;
    use crate::governance::{ActiveWorkOrder, WorkOrder};

    /// An admitted order over `paths` promising to change `symbols`.
    /// Shared with the localization tests so both halves of the gate are
    /// exercised against the same shape of order.
    pub(in crate::governance::gate) fn active(paths: &[&str], symbols: &[&str]) -> ActiveWorkOrder {
        ActiveWorkOrder::for_test(
            WorkOrder {
                id: "wo-1".into(),
                goal: "make parse fallible".into(),
                writable_paths: paths.iter().map(PathBuf::from).collect(),
                target_symbols: symbols.iter().map(|s| (*s).to_string()).collect(),
                evidence_ids: vec!["ev-1".into()],
                acceptance_commands: vec![AcceptanceCommand {
                    command: "cargo test".into(),
                    description: "unit tests".into(),
                }],
            },
            Vec::new(),
        )
    }

    /// A request that passes every rule; each test spoils exactly one fact.
    fn request(order: &ActiveWorkOrder, rust: bool) -> MutationRequest<'_> {
        MutationRequest {
            path: Path::new("src/lib.rs"),
            identity: Some("src/lib.rs"),
            order: Some(order),
            changed: Some((2, 3)),
            evidence: EvidenceState::Fresh {
                id: "ev-1".into(),
                covered: Some((1, 10)),
            },
            rust,
        }
    }

    fn proven(symbols: &[&str]) -> RustFacts {
        RustFacts {
            health: SemanticHealth::Ready,
            outline: Some(symbols.iter().map(|s| (*s).to_string()).collect()),
            semantic: SemanticEvidence::Resolved {
                symbols: symbols.iter().map(|s| (*s).to_string()).collect(),
            },
        }
    }

    #[test]
    fn passes_when_scope_evidence_and_symbol_all_hold() {
        let order = active(&["src/lib.rs"], &["parse"]);
        assert_eq!(
            GateEngine::authorize(&request(&order, true), Some(&proven(&["parse", "render"]))),
            GateDecision::Pass
        );
    }

    #[test]
    fn rejects_mutation_without_a_work_order() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, true);
        req.order = None;
        assert_eq!(
            GateEngine::prescreen(&req),
            GateDecision::Fail(GateFailure::NoWorkOrder)
        );
    }

    #[test]
    fn rejects_path_outside_the_active_scope() {
        let order = active(&["src/other.rs"], &["parse"]);
        let decision = GateEngine::prescreen(&request(&order, true));
        assert!(
            matches!(&decision, GateDecision::Fail(GateFailure::OutOfScope { path, .. })
                if path == Path::new("src/lib.rs")),
            "{decision:?}"
        );
    }

    #[test]
    fn rejects_path_that_resolves_outside_the_project_root() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, true);
        req.identity = None;
        assert!(matches!(
            GateEngine::prescreen(&req),
            GateDecision::Fail(GateFailure::OutsideRoot { .. })
        ));
    }

    #[test]
    fn unread_and_stale_files_ask_for_evidence_rather_than_failing_outright() {
        let order = active(&["src/lib.rs"], &["parse"]);

        let mut missing = request(&order, true);
        missing.evidence = EvidenceState::Missing;
        assert!(matches!(
            GateEngine::prescreen(&missing),
            GateDecision::NeedsEvidence(GateFailure::NoEvidence { .. })
        ));

        let mut stale = request(&order, true);
        stale.evidence = EvidenceState::Stale;
        assert!(matches!(
            GateEngine::prescreen(&stale),
            GateDecision::NeedsEvidence(GateFailure::StaleEvidence { .. })
        ));
    }

    #[test]
    fn rejects_edits_to_lines_the_run_never_read() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, true);
        req.changed = Some((9, 12)); // evidence covers 1-10
        let decision = GateEngine::prescreen(&req);
        assert!(
            matches!(
                decision,
                GateDecision::NeedsEvidence(GateFailure::RangeNotCovered { .. })
            ),
            "{decision:?}"
        );
    }

    #[test]
    fn whole_file_writes_need_whole_file_evidence() {
        let order = active(&["src/lib.rs"], &[]);
        let mut bounded = request(&order, false);
        bounded.changed = None; // whole-file rewrite
        assert!(matches!(
            GateEngine::prescreen(&bounded),
            GateDecision::NeedsEvidence(GateFailure::RangeNotCovered { .. })
        ));

        let mut whole = bounded;
        whole.evidence = EvidenceState::Fresh {
            id: "ev-1".into(),
            covered: None,
        };
        assert_eq!(GateEngine::prescreen(&whole), GateDecision::Pass);
    }

    /// Naming no target symbol is a work-order defect, so it is answered
    /// before anyone waits on a language server.
    #[test]
    fn rust_edits_without_a_target_symbol_fail_before_any_semantic_work() {
        let order = active(&["src/lib.rs"], &[]);
        assert_eq!(
            GateEngine::prescreen(&request(&order, true)),
            GateDecision::Fail(GateFailure::NoTargetSymbol)
        );
    }

    #[test]
    fn non_rust_targets_need_no_semantic_proof() {
        let order = active(&["src/lib.rs"], &[]);
        assert_eq!(
            GateEngine::authorize(&request(&order, false), None),
            GateDecision::Pass
        );
    }

    /// Composition must not become a loophole: a Rust change whose
    /// semantic facts were never gathered is unproven, not authorized.
    #[test]
    fn a_rust_change_with_no_gathered_facts_is_refused() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let decision = GateEngine::authorize(&request(&order, true), None);
        assert!(
            matches!(
                decision,
                GateDecision::Fail(GateFailure::SemanticEvidenceUnavailable { .. })
            ),
            "{decision:?}"
        );
    }

    #[test]
    fn scope_is_checked_before_evidence_so_the_model_fixes_the_order_first() {
        let order = active(&["src/other.rs"], &["parse"]);
        let mut req = request(&order, true);
        req.evidence = EvidenceState::Missing;
        assert!(matches!(
            GateEngine::prescreen(&req),
            GateDecision::Fail(GateFailure::OutOfScope { .. })
        ));
    }

    #[test]
    fn decisions_convert_to_typed_results() {
        assert!(GateDecision::Pass.into_result().is_ok());
        assert_eq!(
            GateDecision::Fail(GateFailure::NoWorkOrder)
                .into_result()
                .unwrap_err(),
            GateFailure::NoWorkOrder
        );
        let stale = GateFailure::StaleEvidence {
            path: PathBuf::from("src/lib.rs"),
        };
        assert_eq!(
            GateDecision::NeedsEvidence(stale.clone())
                .into_result()
                .unwrap_err(),
            stale
        );
    }
}
