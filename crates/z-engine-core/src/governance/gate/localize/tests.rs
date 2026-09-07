//! Localization tests: which symbol authorizes a patch, and which
//! lines that authorization actually covers.

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

/// Symbols the provider placed at given extents.
fn placed(symbols: &[(&str, (u32, u32))]) -> SemanticEvidence {
    SemanticEvidence::Resolved {
        symbols: symbols
            .iter()
            .map(|(name, range)| SymbolExtent {
                name: (*name).to_string(),
                container: None,
                range: *range,
            })
            .collect(),
    }
}

/// The shape most tests want: each symbol spans the same wide extent,
/// so only the tests that are about extents depend on them.
fn resolved(symbols: &[&str]) -> SemanticEvidence {
    placed(
        &symbols
            .iter()
            .map(|name| (*name, (1u32, 100u32)))
            .collect::<Vec<_>>(),
    )
}

fn path() -> PathBuf {
    PathBuf::from("src/lib.rs")
}

/// A change well inside every wide extent above.
const INSIDE: Option<LineRange> = Some((2, 3));

#[test]
fn semantic_evidence_authorizes_a_symbol_both_sources_agree_on() {
    let order = active(&["src/lib.rs"], &["parse"]);
    assert_eq!(
        localize(
            &facts(Some(&["parse", "render"]), resolved(&["parse", "render"])),
            &order,
            &path(),
            INSIDE
        ),
        GateDecision::Pass
    );
}

/// The load-bearing case for finding I2: `parse` and `render` both
/// live in this file, the order names only `parse`, and the patch
/// lands in `render`. Naming one symbol must not license editing its
/// neighbour.
#[test]
fn an_edit_to_a_sibling_function_is_refused_even_though_the_target_lives_here() {
    let order = active(&["src/lib.rs"], &["parse"]);
    let facts = facts(
        Some(&["parse", "render"]),
        placed(&[("parse", (1, 9)), ("render", (11, 20))]),
    );

    let inside = localize(&facts, &order, &path(), Some((3, 4)));
    assert_eq!(inside, GateDecision::Pass, "the target itself is editable");

    let sibling = localize(&facts, &order, &path(), Some((13, 14)));
    let GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol {
        changed, symbols, ..
    }) = &sibling
    else {
        panic!("editing an undeclared neighbour must be refused: {sibling:?}");
    };
    assert_eq!(changed, "lines 13-14");
    assert_eq!(symbols, "parse (lines 1-9)", "the refusal names the extent");

    // …and a change that merely *overlaps* the target is not inside it.
    let straddling = localize(&facts, &order, &path(), Some((8, 13)));
    assert!(
        matches!(
            straddling,
            GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol { .. })
        ),
        "{straddling:?}"
    );
}

/// The file header, imports, and the gap between two functions belong
/// to no declaration, so nothing authorizes changing them.
#[test]
fn lines_between_declarations_belong_to_no_symbol() {
    let order = active(&["src/lib.rs"], &["parse", "render"]);
    let facts = facts(
        Some(&["parse", "render"]),
        placed(&[("parse", (5, 9)), ("render", (11, 20))]),
    );
    for changed in [Some((1, 1)), Some((10, 10)), Some((21, 30))] {
        assert!(
            matches!(
                localize(&facts, &order, &path(), changed),
                GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol { .. })
            ),
            "{changed:?} is outside every declared symbol"
        );
    }
}

/// A whole-file write is not a change to a symbol, whatever the order
/// names — a file is more than its declarations.
#[test]
fn a_whole_file_rust_write_cannot_be_bound_to_a_symbol() {
    let order = active(&["src/lib.rs"], &["parse"]);
    let decision = localize(
        &facts(Some(&["parse"]), placed(&[("parse", (1, 9))])),
        &order,
        &path(),
        None,
    );
    assert!(
        matches!(
            decision,
            GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol { .. })
        ),
        "{decision:?}"
    );
}

/// Every changed line has to be inside *one* symbol, not spread
/// across two — a patch spanning both is a patch to the gap as well.
#[test]
fn a_change_spanning_two_declared_symbols_is_still_refused() {
    let order = active(&["src/lib.rs"], &["parse", "render"]);
    let facts = facts(
        Some(&["parse", "render"]),
        placed(&[("parse", (1, 9)), ("render", (10, 20))]),
    );
    assert_eq!(
        localize(&facts, &order, &path(), Some((11, 12))),
        GateDecision::Pass,
        "the second target is editable on its own"
    );
    assert!(matches!(
        localize(&facts, &order, &path(), Some((8, 12))),
        GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol { .. })
    ));
}

/// A qualified target pins the container the provider reported, so
/// `Parser::run` never authorizes a free `run` beside it.
#[test]
fn a_qualified_target_does_not_authorize_a_free_function_of_the_same_name() {
    let order = active(&["src/lib.rs"], &["Parser::run"]);
    let semantic = SemanticEvidence::Resolved {
        symbols: vec![
            SymbolExtent {
                name: "run".into(),
                container: None,
                range: (1, 9),
            },
            SymbolExtent {
                name: "run".into(),
                container: Some("Parser".into()),
                range: (11, 20),
            },
        ],
    };
    let facts = facts(Some(&["run", "Parser"]), semantic);
    assert_eq!(
        localize(&facts, &order, &path(), Some((12, 13))),
        GateDecision::Pass,
        "the method the order named is editable"
    );
    assert!(
        matches!(
            localize(&facts, &order, &path(), Some((3, 4))),
            GateDecision::Fail(GateFailure::ChangeOutsideTargetSymbol { .. })
        ),
        "a free function of the same name is a different symbol"
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
        INSIDE,
    );
    assert!(
        matches!(
            decision,
            GateDecision::Fail(GateFailure::UnresolvedTargetSymbol { .. })
        ),
        "{decision:?}"
    );
}

/// …and it may still only narrow: an outline that does not mention the
/// target refuses before the semantic answer is weighed.
#[test]
fn a_tree_sitter_outline_can_still_only_narrow() {
    let order = active(&["src/lib.rs"], &["parse"]);
    let decision = localize(
        &facts(Some(&["render"]), placed(&[("parse", (1, 9))])),
        &order,
        &path(),
        Some((3, 4)),
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
        INSIDE,
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
        INSIDE,
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
        localize(&unhealthy, &order, &path(), INSIDE),
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
            &path(),
            INSIDE
        ),
        GateDecision::Fail(GateFailure::NoTargetSymbol)
    );
}

#[test]
fn a_missing_outline_narrows_nothing_and_semantics_still_decide() {
    let order = active(&["src/lib.rs"], &["WorkOrder::parse"]);
    let semantic = SemanticEvidence::Resolved {
        symbols: vec![SymbolExtent {
            name: "parse".into(),
            container: Some("WorkOrder".into()),
            range: (1, 20),
        }],
    };
    assert_eq!(
        localize(&facts(None, semantic), &order, &path(), INSIDE),
        GateDecision::Pass,
        "qualified targets resolve by their final segment"
    );
    assert!(matches!(
        localize(&facts(None, resolved(&["render"])), &order, &path(), INSIDE),
        GateDecision::Fail(GateFailure::UnresolvedTargetSymbol { .. })
    ));
}
