//! The tools layer's port onto Rust semantics.
//!
//! The mutation gate needs two answers — "can a Rust semantic provider
//! answer questions right now?" and "what does it say this file
//! declares?" — and must get both without depending on the language
//! server client's concrete API. [`RustSemantics`] is that seam:
//! `LspClient` implements it in production, and tests substitute a
//! scripted stub so gating never depends on rust-analyzer being
//! installed, nor on how a subprocess is spawned.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use crate::lsp::{LspClient, LspHealth, SymbolAnswer};

use super::ToolCtx;

/// Something that can answer Rust semantic questions about this run.
#[async_trait]
pub trait RustSemantics: Send + Sync {
    /// Is the provider able to answer at all?
    async fn health(&self) -> LspHealth;

    /// Which symbols does the provider say `text` declares in `abs_path`?
    /// Implementations must distinguish "nothing indexed" from "answered
    /// about another file" — the gate refuses on both, but says why.
    async fn document_symbols(&self, abs_path: &Path, text: &str) -> SymbolAnswer;
}

#[async_trait]
impl RustSemantics for LspClient {
    async fn health(&self) -> LspHealth {
        LspClient::health(self).await
    }

    async fn document_symbols(&self, abs_path: &Path, text: &str) -> SymbolAnswer {
        LspClient::document_symbols(self, abs_path, text).await
    }
}

impl ToolCtx {
    /// Attach the run's language server. Both handles point at the same
    /// client: `lsp` for location/diagnostic requests, `semantics` for the
    /// gate's questions — set together so a run can never gate against a
    /// different server than it queries.
    pub fn attach_lsp(&mut self, client: Arc<LspClient>) {
        self.semantics = Some(Arc::clone(&client) as Arc<dyn RustSemantics>);
        self.lsp = Some(client);
    }

    /// Health of this run's Rust semantic provider. No provider attached
    /// is *not* healthy — an absent server proves nothing.
    pub(super) async fn semantic_health(&self) -> LspHealth {
        match &self.semantics {
            Some(provider) => provider.health().await,
            None => LspHealth::Unavailable(
                "no Rust semantic provider is attached to this run".to_string(),
            ),
        }
    }

    /// Semantic declarations for `abs_path`, as the provider sees the
    /// exact `text` about to be overwritten. No provider means no
    /// evidence, which the gate treats as a refusal.
    pub(super) async fn semantic_symbols(&self, abs_path: &Path, text: &str) -> SymbolAnswer {
        match &self.semantics {
            Some(provider) => provider.document_symbols(abs_path, text).await,
            None => SymbolAnswer::Unindexed(
                "no Rust semantic provider is attached to this run".to_string(),
            ),
        }
    }
}

/// Scripted provider for tests, so gating never spawns rust-analyzer and
/// every answer shape — including the refusals — is reachable.
#[cfg(test)]
pub(crate) struct StubSemantics {
    pub(crate) health: LspHealth,
    pub(crate) answer: SymbolAnswer,
}

#[cfg(test)]
impl StubSemantics {
    /// A healthy provider that resolves exactly `symbols` for any file.
    pub(crate) fn resolving(symbols: &[&str]) -> Self {
        Self {
            health: LspHealth::Ready,
            answer: SymbolAnswer::Resolved(symbols.iter().map(|s| (*s).to_string()).collect()),
        }
    }

    /// A healthy provider that gives `answer` — used for the unindexed
    /// and mismatched refusals.
    pub(crate) fn answering(answer: SymbolAnswer) -> Self {
        Self {
            health: LspHealth::Ready,
            answer,
        }
    }

    /// A provider that cannot answer at all.
    pub(crate) fn unavailable(reason: &str) -> Self {
        Self {
            health: LspHealth::Unavailable(reason.to_string()),
            answer: SymbolAnswer::Unindexed(reason.to_string()),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl RustSemantics for StubSemantics {
    async fn health(&self) -> LspHealth {
        self.health.clone()
    }

    async fn document_symbols(&self, _abs_path: &Path, _text: &str) -> SymbolAnswer {
        self.answer.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_context_without_a_provider_is_never_healthy() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = super::super::test_support::plain_ctx(tmp.path());
        assert!(!ctx.semantic_health().await.is_ready());
    }

    #[tokio::test]
    async fn a_context_without_a_provider_yields_no_symbol_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = super::super::test_support::plain_ctx(tmp.path());
        assert!(matches!(
            ctx.semantic_symbols(&tmp.path().join("src/lib.rs"), "fn parse() {}")
                .await,
            SymbolAnswer::Unindexed(_)
        ));
    }

    #[tokio::test]
    async fn an_attached_provider_answers_for_the_context() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = super::super::test_support::plain_ctx(tmp.path());
        ctx.semantics = Some(Arc::new(StubSemantics::resolving(&["parse"])));
        assert!(ctx.semantic_health().await.is_ready());
        assert_eq!(
            ctx.semantic_symbols(&tmp.path().join("src/lib.rs"), "fn parse() {}")
                .await,
            SymbolAnswer::Resolved(vec!["parse".to_string()])
        );
    }
}
