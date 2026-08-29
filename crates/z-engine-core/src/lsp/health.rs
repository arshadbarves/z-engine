//! Explicit health for the Rust semantic provider.
//!
//! The gate must never treat "unknown" as healthy, so health is a value
//! the client computes on demand rather than a flag callers can forget to
//! check: asking brings the server up if it is not running and reports
//! the exact reason when it cannot.

use super::LspClient;

/// Whether rust-analyzer can answer semantic questions right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspHealth {
    /// Initialized and accepting requests.
    Ready,
    /// Unusable; the string is the model-visible reason.
    Unavailable(String),
}

impl LspHealth {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl LspClient {
    /// Ensure a connection and report whether the server is usable.
    ///
    /// Cheap once initialized (`ensure` short-circuits on the ready
    /// flag); the first call pays the spawn + handshake, and a crashed
    /// server is re-spawned up to the client's bounded attempt limit.
    pub async fn health(&self) -> LspHealth {
        match self.ensure().await {
            Ok(()) => LspHealth::Ready,
            Err(reason) => LspHealth::Unavailable(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn a_server_that_cannot_spawn_reports_why_instead_of_ready() {
        let tmp = tempfile::tempdir().unwrap();
        let client = LspClient::new(
            tmp.path(),
            PathBuf::from("definitely-not-a-language-server"),
        );
        let health = client.health().await;
        assert!(
            matches!(&health, LspHealth::Unavailable(reason) if !reason.is_empty()),
            "{health:?}"
        );
        assert!(!health.is_ready());
    }
}
