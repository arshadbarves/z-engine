//! The rules. Given [`MutationRequest`] facts, decide — in the order a
//! reviewer would ask — whether the change may touch the working tree.

use std::path::{Path, PathBuf};

use crate::governance::ActiveWorkOrder;

use super::facts::{EvidenceState, LineRange, MutationRequest, RustFacts, SemanticHealth};
use super::failure::GateFailure;

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
}

/// The pure decision procedure. No I/O, no clocks, no globals.
#[derive(Debug)]
pub struct GateEngine;

impl GateEngine {
    /// Decide whether one mutation may touch the working tree.
    pub fn authorize(req: &MutationRequest<'_>) -> GateDecision {
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
        if let Some(blocked) = evidence_gap(&req.evidence, req.changed, scoped) {
            return GateDecision::NeedsEvidence(blocked);
        }
        match &req.rust {
            None => GateDecision::Pass,
            Some(rust) => authorize_rust(rust, order, scoped),
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

/// Localization for Rust source: a healthy provider, and a declared
/// target symbol that really lives in this file.
fn authorize_rust(rust: &RustFacts, order: &ActiveWorkOrder, path: &Path) -> GateDecision {
    if let SemanticHealth::Unavailable { reason } = &rust.health {
        return GateDecision::Fail(GateFailure::SemanticProviderUnavailable {
            reason: reason.clone(),
        });
    }
    let targets = &order.order.target_symbols;
    if targets.is_empty() {
        return GateDecision::Fail(GateFailure::NoTargetSymbol);
    }
    if targets
        .iter()
        .any(|t| rust.declared.iter().any(|d| d == leaf(t)))
    {
        GateDecision::Pass
    } else {
        GateDecision::Fail(GateFailure::UnresolvedTargetSymbol {
            symbols: targets.join(", "),
            path: path.to_path_buf(),
        })
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

/// Final segment of a possibly qualified symbol name (`Type::method` →
/// `method`), which is what a file-level outline can declare.
fn leaf(symbol: &str) -> &str {
    symbol.rsplit("::").next().unwrap_or(symbol).trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::work_order::AcceptanceCommand;
    use crate::governance::{ActiveWorkOrder, WorkOrder};

    fn active(paths: &[&str], symbols: &[&str]) -> ActiveWorkOrder {
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
    fn request<'a>(order: &'a ActiveWorkOrder, rust: Option<RustFacts>) -> MutationRequest<'a> {
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

    fn healthy(symbols: &[&str]) -> Option<RustFacts> {
        Some(RustFacts {
            health: SemanticHealth::Ready,
            declared: symbols.iter().map(|s| (*s).to_string()).collect(),
        })
    }

    #[test]
    fn passes_when_scope_evidence_and_symbol_all_hold() {
        let order = active(&["src/lib.rs"], &["parse"]);
        assert_eq!(
            GateEngine::authorize(&request(&order, healthy(&["parse", "render"]))),
            GateDecision::Pass
        );
    }

    #[test]
    fn rejects_mutation_without_a_work_order() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, healthy(&["parse"]));
        req.order = None;
        assert_eq!(
            GateEngine::authorize(&req),
            GateDecision::Fail(GateFailure::NoWorkOrder)
        );
    }

    #[test]
    fn rejects_path_outside_the_active_scope() {
        let order = active(&["src/other.rs"], &["parse"]);
        let decision = GateEngine::authorize(&request(&order, healthy(&["parse"])));
        assert!(
            matches!(&decision, GateDecision::Fail(GateFailure::OutOfScope { path, .. })
                if path == Path::new("src/lib.rs")),
            "{decision:?}"
        );
    }

    #[test]
    fn rejects_path_that_resolves_outside_the_project_root() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, healthy(&["parse"]));
        req.identity = None;
        assert!(matches!(
            GateEngine::authorize(&req),
            GateDecision::Fail(GateFailure::OutsideRoot { .. })
        ));
    }

    #[test]
    fn unread_and_stale_files_ask_for_evidence_rather_than_failing_outright() {
        let order = active(&["src/lib.rs"], &["parse"]);

        let mut missing = request(&order, healthy(&["parse"]));
        missing.evidence = EvidenceState::Missing;
        assert!(matches!(
            GateEngine::authorize(&missing),
            GateDecision::NeedsEvidence(GateFailure::NoEvidence { .. })
        ));

        let mut stale = request(&order, healthy(&["parse"]));
        stale.evidence = EvidenceState::Stale;
        assert!(matches!(
            GateEngine::authorize(&stale),
            GateDecision::NeedsEvidence(GateFailure::StaleEvidence { .. })
        ));
    }

    #[test]
    fn rejects_edits_to_lines_the_run_never_read() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, healthy(&["parse"]));
        req.changed = Some((9, 12)); // evidence covers 1-10
        let decision = GateEngine::authorize(&req);
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
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut bounded = request(&order, healthy(&["parse"]));
        bounded.changed = None; // whole-file rewrite
        assert!(matches!(
            GateEngine::authorize(&bounded),
            GateDecision::NeedsEvidence(GateFailure::RangeNotCovered { .. })
        ));

        let mut whole = bounded;
        whole.evidence = EvidenceState::Fresh {
            id: "ev-1".into(),
            covered: None,
        };
        assert_eq!(GateEngine::authorize(&whole), GateDecision::Pass);
    }

    #[test]
    fn rejects_rust_edits_when_the_semantic_provider_is_unhealthy() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut req = request(&order, healthy(&["parse"]));
        req.rust = Some(RustFacts {
            health: SemanticHealth::Unavailable {
                reason: "spawn rust-analyzer: not found".into(),
            },
            declared: vec!["parse".into()],
        });
        let decision = GateEngine::authorize(&req);
        assert!(
            matches!(
                decision,
                GateDecision::Fail(GateFailure::SemanticProviderUnavailable { .. })
            ),
            "{decision:?}"
        );
    }

    #[test]
    fn rejects_rust_edits_whose_target_symbol_is_not_declared_here() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let decision = GateEngine::authorize(&request(&order, healthy(&["render", "main"])));
        assert!(
            matches!(
                decision,
                GateDecision::Fail(GateFailure::UnresolvedTargetSymbol { .. })
            ),
            "{decision:?}"
        );
    }

    #[test]
    fn rejects_rust_edits_when_the_order_names_no_target_symbol() {
        let order = active(&["src/lib.rs"], &[]);
        assert_eq!(
            GateEngine::authorize(&request(&order, healthy(&["parse"]))),
            GateDecision::Fail(GateFailure::NoTargetSymbol)
        );
    }

    #[test]
    fn qualified_target_symbols_resolve_by_their_final_segment() {
        let order = active(&["src/lib.rs"], &["WorkOrder::validate"]);
        assert_eq!(
            GateEngine::authorize(&request(&order, healthy(&["validate"]))),
            GateDecision::Pass
        );
    }

    #[test]
    fn non_rust_targets_need_no_semantic_proof() {
        let order = active(&["src/lib.rs"], &[]);
        assert_eq!(
            GateEngine::authorize(&request(&order, None)),
            GateDecision::Pass
        );
    }

    #[test]
    fn scope_is_checked_before_evidence_so_the_model_fixes_the_order_first() {
        let order = active(&["src/other.rs"], &["parse"]);
        let mut req = request(&order, healthy(&["parse"]));
        req.evidence = EvidenceState::Missing;
        assert!(matches!(
            GateEngine::authorize(&req),
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
