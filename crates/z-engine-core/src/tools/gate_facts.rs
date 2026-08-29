//! Gathering the Rust facts the mutation gate judges.
//!
//! Two sources, deliberately unequal. The tree-sitter *outline* describes
//! the text the model is about to overwrite; it is cheap, local, and can
//! only ever narrow candidates. The *semantic* answer comes from the
//! language server behind [`RustSemantics`] and is the only thing that can
//! authorize a Rust mutation — so every way it can fail to describe this
//! file (unavailable, unindexed, answered about something else, or not
//! even text) is carried through as a distinct refusal rather than being
//! flattened into an empty symbol list.
//!
//! Split out of `gate_ctx` because gathering facts and applying a verdict
//! are different reasons to change.

use std::path::Path;

use crate::governance::{RustFacts, SemanticEvidence, SemanticHealth};
use crate::lsp::{LspHealth, SymbolAnswer};

use super::ToolCtx;

impl ToolCtx {
    /// Everything [`crate::governance::GateEngine::localize`] needs about
    /// `path`, given the exact `current` bytes being replaced.
    ///
    /// Only called once the semantics-free rules have passed, so an
    /// unusable work order never costs a language-server round trip.
    pub(super) async fn rust_facts(&self, path: &Path, current: &[u8]) -> RustFacts {
        let health = semantic_health(self.semantic_health().await);
        let text = std::str::from_utf8(current).ok();
        let semantic = match (&health, text) {
            (SemanticHealth::Unavailable { reason }, _) => SemanticEvidence::Unindexed {
                reason: reason.clone(),
            },
            (_, None) => SemanticEvidence::Mismatched {
                reason: "the bytes being changed are not valid UTF-8, so no semantic answer \
                         can describe them"
                    .into(),
            },
            (_, Some(text)) => evidence(self.semantic_symbols(&self.resolve(path), text).await),
        };
        RustFacts {
            health,
            outline: text.and_then(outline_symbols),
            semantic,
        }
    }
}

/// Rust source is the only content this slice makes semantic claims about.
pub(super) fn is_rust(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
}

/// Symbols the repo map's tree-sitter outline finds — the project's one
/// Rust symbol extractor. `None` when the text does not parse, in which
/// case the outline narrows nothing and semantics decide alone.
fn outline_symbols(text: &str) -> Option<Vec<String>> {
    crate::context::repo_map::extract_rust(text)
        .map(|outline| outline.symbols.into_iter().map(|s| s.name).collect())
}

fn evidence(answer: SymbolAnswer) -> SemanticEvidence {
    match answer {
        SymbolAnswer::Resolved(symbols) => SemanticEvidence::Resolved { symbols },
        SymbolAnswer::Unindexed(reason) => SemanticEvidence::Unindexed { reason },
        SymbolAnswer::Mismatched(reason) => SemanticEvidence::Mismatched { reason },
    }
}

fn semantic_health(health: LspHealth) -> SemanticHealth {
    match health {
        LspHealth::Ready => SemanticHealth::Ready,
        LspHealth::Unavailable(reason) => SemanticHealth::Unavailable { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::semantics::StubSemantics;
    use crate::tools::test_support::plain_ctx;
    use std::sync::Arc;

    const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";

    fn ctx_with(stub: StubSemantics) -> (ToolCtx, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = plain_ctx(tmp.path());
        ctx.semantics = Some(Arc::new(stub));
        (ctx, tmp)
    }

    #[test]
    fn only_rust_paths_carry_a_semantic_claim() {
        assert!(is_rust(Path::new("src/lib.rs")));
        assert!(is_rust(Path::new("SRC/LIB.RS")));
        assert!(!is_rust(Path::new("notes.md")));
        assert!(!is_rust(Path::new("rs")));
    }

    #[tokio::test]
    async fn facts_carry_both_views_without_conflating_them() {
        let (ctx, tmp) = ctx_with(StubSemantics::resolving(&["parse"]));
        let facts = ctx
            .rust_facts(&tmp.path().join("lib.rs"), LIB.as_bytes())
            .await;
        assert_eq!(facts.health, SemanticHealth::Ready);
        assert_eq!(facts.outline.as_deref(), Some(&["parse".to_string()][..]));
        assert_eq!(
            facts.semantic,
            SemanticEvidence::Resolved {
                symbols: vec!["parse".to_string()]
            }
        );
    }

    /// An unavailable provider must not be asked, and must not be recorded
    /// as an empty (therefore falsifiable) semantic answer.
    #[tokio::test]
    async fn an_unavailable_provider_yields_no_semantic_evidence() {
        let (ctx, tmp) = ctx_with(StubSemantics::unavailable("spawn rust-analyzer: not found"));
        let facts = ctx
            .rust_facts(&tmp.path().join("lib.rs"), LIB.as_bytes())
            .await;
        assert!(matches!(facts.health, SemanticHealth::Unavailable { .. }));
        assert!(matches!(facts.semantic, SemanticEvidence::Unindexed { .. }));
    }

    #[tokio::test]
    async fn a_foreign_answer_is_carried_through_as_a_mismatch() {
        let (ctx, tmp) = ctx_with(StubSemantics::answering(SymbolAnswer::Mismatched(
            "symbols were reported for file:///other.rs".into(),
        )));
        let facts = ctx
            .rust_facts(&tmp.path().join("lib.rs"), LIB.as_bytes())
            .await;
        assert!(matches!(
            facts.semantic,
            SemanticEvidence::Mismatched { .. }
        ));
    }

    #[tokio::test]
    async fn bytes_that_are_not_text_can_never_be_localized() {
        let (ctx, tmp) = ctx_with(StubSemantics::resolving(&["parse"]));
        let facts = ctx
            .rust_facts(&tmp.path().join("lib.rs"), &[0xff, 0xfe])
            .await;
        assert_eq!(facts.outline, None);
        assert!(matches!(
            facts.semantic,
            SemanticEvidence::Mismatched { .. }
        ));
    }
}
