//! Localization for Rust source: proving that the symbol a work order
//! promised to change actually lives, semantically, in the file about to
//! be written.
//!
//! The order of proof matters and is deliberate:
//!
//! 1. The semantic provider must be **healthy** — an absent or broken
//!    rust-analyzer proves nothing, so it blocks.
//! 2. The tree-sitter outline may **narrow**: if the text does not even
//!    mention the target as a declaration, stop here without paying for a
//!    semantic round trip's worth of doubt. It can only ever refuse.
//! 3. The provider must have **answered about this document**, and its
//!    answer must contain the target. This is the only step that
//!    authorizes; "not indexed yet" and "answered about another file" are
//!    refusals, because an unproven claim is exactly what the gate exists
//!    to stop.

use std::path::Path;

use crate::governance::ActiveWorkOrder;

use super::engine::GateDecision;
use super::facts::{RustFacts, SemanticEvidence, SemanticHealth};
use super::failure::GateFailure;

/// Decide whether `facts` localize the order's target symbols in `path`.
pub(super) fn localize(facts: &RustFacts, order: &ActiveWorkOrder, path: &Path) -> GateDecision {
    let targets = &order.order.target_symbols;
    if targets.is_empty() {
        return GateDecision::Fail(GateFailure::NoTargetSymbol);
    }
    if let SemanticHealth::Unavailable { reason } = &facts.health {
        return GateDecision::Fail(GateFailure::SemanticProviderUnavailable {
            reason: reason.clone(),
        });
    }
    // Tree-sitter narrows; it never authorizes. An outline that has no
    // opinion (unparseable or absent) simply narrows nothing.
    if let Some(outline) = &facts.outline {
        if !declares(outline, targets) {
            return unresolved(targets, path);
        }
    }
    match &facts.semantic {
        SemanticEvidence::Unindexed { reason } => {
            GateDecision::Fail(GateFailure::SemanticEvidenceUnavailable {
                path: path.to_path_buf(),
                reason: reason.clone(),
            })
        }
        SemanticEvidence::Mismatched { reason } => {
            GateDecision::Fail(GateFailure::SemanticEvidenceMismatch {
                path: path.to_path_buf(),
                reason: reason.clone(),
            })
        }
        SemanticEvidence::Resolved { symbols } if declares(symbols, targets) => GateDecision::Pass,
        SemanticEvidence::Resolved { .. } => unresolved(targets, path),
    }
}

fn unresolved(targets: &[String], path: &Path) -> GateDecision {
    GateDecision::Fail(GateFailure::UnresolvedTargetSymbol {
        symbols: targets.join(", "),
        path: path.to_path_buf(),
    })
}

/// Does any target appear among `declared`? Symbols are compared by their
/// final `::` segment, which is what a file-level symbol list reports.
fn declares(declared: &[String], targets: &[String]) -> bool {
    targets
        .iter()
        .any(|t| declared.iter().any(|d| leaf(d) == leaf(t)))
}

fn leaf(symbol: &str) -> &str {
    symbol.rsplit("::").next().unwrap_or(symbol).trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::gate::engine::tests::active;
    use std::path::PathBuf;

    fn facts(outline: Option<&[&str]>, semantic: SemanticEvidence) -> RustFacts {
        RustFacts {
            health: SemanticHealth::Ready,
            outline: outline.map(|names| names.iter().map(|s| (*s).to_string()).collect()),
            semantic,
        }
    }

    fn resolved(symbols: &[&str]) -> SemanticEvidence {
        SemanticEvidence::Resolved {
            symbols: symbols.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn path() -> PathBuf {
        PathBuf::from("src/lib.rs")
    }

    #[test]
    fn semantic_evidence_authorizes_a_symbol_both_sources_agree_on() {
        let order = active(&["src/lib.rs"], &["parse"]);
        assert_eq!(
            localize(
                &facts(Some(&["parse", "render"]), resolved(&["parse", "render"])),
                &order,
                &path()
            ),
            GateDecision::Pass
        );
    }

    /// The load-bearing case: tree-sitter sees the symbol, rust-analyzer
    /// does not. A text outline cannot authorize, so this must refuse.
    #[test]
    fn a_tree_sitter_outline_alone_never_authorizes() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let decision = localize(
            &facts(Some(&["parse"]), resolved(&["render"])),
            &order,
            &path(),
        );
        assert!(
            matches!(
                decision,
                GateDecision::Fail(GateFailure::UnresolvedTargetSymbol { .. })
            ),
            "{decision:?}"
        );
    }

    #[test]
    fn an_unindexed_or_foreign_answer_is_a_refusal_not_an_empty_pass() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let unindexed = localize(
            &facts(
                Some(&["parse"]),
                SemanticEvidence::Unindexed {
                    reason: "documentSymbol timed out".into(),
                },
            ),
            &order,
            &path(),
        );
        assert!(
            matches!(
                unindexed,
                GateDecision::Fail(GateFailure::SemanticEvidenceUnavailable { .. })
            ),
            "{unindexed:?}"
        );

        let foreign = localize(
            &facts(
                Some(&["parse"]),
                SemanticEvidence::Mismatched {
                    reason: "symbols were reported for file:///other.rs".into(),
                },
            ),
            &order,
            &path(),
        );
        assert!(
            matches!(
                foreign,
                GateDecision::Fail(GateFailure::SemanticEvidenceMismatch { .. })
            ),
            "{foreign:?}"
        );
    }

    #[test]
    fn an_unhealthy_provider_blocks_before_any_symbol_is_weighed() {
        let order = active(&["src/lib.rs"], &["parse"]);
        let mut unhealthy = facts(Some(&["parse"]), resolved(&["parse"]));
        unhealthy.health = SemanticHealth::Unavailable {
            reason: "spawn rust-analyzer: not found".into(),
        };
        assert!(matches!(
            localize(&unhealthy, &order, &path()),
            GateDecision::Fail(GateFailure::SemanticProviderUnavailable { .. })
        ));
    }

    #[test]
    fn an_order_naming_no_symbol_localizes_nothing() {
        let order = active(&["src/lib.rs"], &[]);
        assert_eq!(
            localize(
                &facts(Some(&["parse"]), resolved(&["parse"])),
                &order,
                &path()
            ),
            GateDecision::Fail(GateFailure::NoTargetSymbol)
        );
    }

    #[test]
    fn a_missing_outline_narrows_nothing_and_semantics_still_decide() {
        let order = active(&["src/lib.rs"], &["WorkOrder::parse"]);
        assert_eq!(
            localize(&facts(None, resolved(&["parse"])), &order, &path()),
            GateDecision::Pass,
            "qualified targets resolve by their final segment"
        );
        assert!(matches!(
            localize(&facts(None, resolved(&["render"])), &order, &path()),
            GateDecision::Fail(GateFailure::UnresolvedTargetSymbol { .. })
        ));
    }
}
