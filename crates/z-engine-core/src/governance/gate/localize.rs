//! Localization for Rust source: proving that the lines about to change
//! lie inside a symbol the work order promised to change.
//!
//! The order of proof matters and is deliberate:
//!
//! 1. The semantic provider must be **healthy** — an absent or broken
//!    rust-analyzer proves nothing, so it blocks.
//! 2. The tree-sitter outline may **narrow**: if the text does not even
//!    mention the target as a declaration, stop here without paying for a
//!    semantic round trip's worth of doubt. It can only ever refuse.
//! 3. The provider must have **answered about this document**, and its
//!    answer must place a target symbol here. This is the only step that
//!    authorizes; "not indexed yet" and "answered about another file" are
//!    refusals, because an unproven claim is exactly what the gate exists
//!    to stop.
//! 4. Every changed line must fall inside the **extent** of one of those
//!    target symbols. Without this the unit of authorization is the file:
//!    naming `parse` would license rewriting the function beside it, which
//!    is precisely the claim the mode is built to make false.
//!
//! Step 4 is why [`SemanticEvidence::Resolved`] carries ranges rather
//! than names. Tree-sitter has start lines but no extents, so it stays
//! where it was — narrowing, never authorizing.

use std::path::Path;

use crate::governance::ActiveWorkOrder;

use super::engine::GateDecision;
use super::facts::{LineRange, RustFacts, SemanticEvidence, SemanticHealth, SymbolExtent};
use super::failure::GateFailure;

/// Decide whether `facts` localize the order's target symbols in `path`,
/// and whether `changed` stays inside them.
pub(super) fn localize(
    facts: &RustFacts,
    order: &ActiveWorkOrder,
    path: &Path,
    changed: Option<LineRange>,
) -> GateDecision {
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
    // opinion (unparseable or absent) simply narrows nothing. It is
    // matched on the leaf name alone, because it knows nothing about
    // containers — narrowing on a qualifier it cannot see would turn a
    // refusal to *narrow* into a refusal to *authorize*.
    if let Some(outline) = &facts.outline {
        if !outline
            .iter()
            .any(|d| targets.iter().any(|t| leaf(t) == leaf(d)))
        {
            return unresolved(targets, path);
        }
    }
    let symbols = match &facts.semantic {
        SemanticEvidence::Unindexed { reason } => {
            return GateDecision::Fail(GateFailure::SemanticEvidenceUnavailable {
                path: path.to_path_buf(),
                reason: reason.clone(),
            });
        }
        SemanticEvidence::Mismatched { reason } => {
            return GateDecision::Fail(GateFailure::SemanticEvidenceMismatch {
                path: path.to_path_buf(),
                reason: reason.clone(),
            });
        }
        SemanticEvidence::Resolved { symbols } => symbols,
    };

    let declared: Vec<&SymbolExtent> = symbols
        .iter()
        .filter(|s| matches_any(&s.name, s.container.as_deref(), targets))
        .collect();
    if declared.is_empty() {
        return unresolved(targets, path);
    }
    if declared.iter().any(|s| s.covers(changed)) {
        return GateDecision::Pass;
    }
    GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol {
        path: path.to_path_buf(),
        changed: label(changed),
        symbols: describe(&declared),
    })
}

fn unresolved(targets: &[String], path: &Path) -> GateDecision {
    GateDecision::Fail(GateFailure::UnresolvedTargetSymbol {
        symbols: targets.join(", "),
        path: path.to_path_buf(),
    })
}

/// Does a declaration named `name` (nested in `container`) answer to any
/// of `targets`?
///
/// Targets are compared by their final `::` segment, which is what a
/// file-level symbol list reports. A *qualified* target additionally
/// pins the container when the provider named one, so `Parser::run` does
/// not authorize a free function called `run` in the same file.
fn matches_any(name: &str, container: Option<&str>, targets: &[String]) -> bool {
    targets.iter().any(|target| {
        if leaf(target) != leaf(name) {
            return false;
        }
        match (qualifier(target), container) {
            (Some(expected), Some(actual)) => leaf(expected) == leaf(actual),
            (Some(_), None) => false,
            (None, _) => true,
        }
    })
}

fn leaf(symbol: &str) -> &str {
    symbol.rsplit("::").next().unwrap_or(symbol).trim()
}

/// The part of `Type::method` before the final `::`, if any.
fn qualifier(symbol: &str) -> Option<&str> {
    symbol.rsplit_once("::").map(|(head, _)| head.trim())
}

fn label(range: Option<LineRange>) -> String {
    match range {
        Some((first, last)) if first == last => format!("line {first}"),
        Some((first, last)) => format!("lines {first}-{last}"),
        None => "the whole file".to_string(),
    }
}

fn describe(declared: &[&SymbolExtent]) -> String {
    declared
        .iter()
        .map(|s| format!("{} (lines {}-{})", s.name, s.range.0, s.range.1))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
