//! The tools layer's port onto Rust semantic health.
//!
//! The mutation gate needs one answer — "can a Rust semantic provider
//! answer questions right now?" — and must get it without depending on
//! the language-server client's concrete API. [`RustSemantics`] is that
//! seam: `LspClient` implements it in production, and tests substitute a
//! stub so gating never depends on rust-analyzer being installed.

use std::sync::Arc;

use async_trait::async_trait;

use crate::lsp::{LspClient, LspHealth};

use super::ToolCtx;

/// Something that can report whether Rust semantics are available.
#[async_trait]
pub trait RustSemantics: Send + Sync {
    async fn health(&self) -> LspHealth;
}

#[async_trait]
impl RustSemantics for LspClient {
    async fn health(&self) -> LspHealth {
        LspClient::health(self).await
    }
}

impl ToolCtx {
    /// Attach the run's language server. Both handles point at the same
    /// client: `lsp` for location/diagnostic requests, `semantics` for the
    /// gate's health question — set together so a run can never gate
    /// against a different server than it queries.
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
}

/// Fixed health answer for tests, so gating never spawns rust-analyzer.
#[cfg(test)]
pub(crate) struct StubSemantics(pub(crate) LspHealth);

#[cfg(test)]
#[async_trait]
impl RustSemantics for StubSemantics {
    async fn health(&self) -> LspHealth {
        self.0.clone()
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
    async fn an_attached_provider_answers_for_the_context() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = super::super::test_support::plain_ctx(tmp.path());
        ctx.semantics = Some(Arc::new(StubSemantics(LspHealth::Ready)));
        assert!(ctx.semantic_health().await.is_ready());
    }
}
