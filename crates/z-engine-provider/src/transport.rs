//! The provider seam: an object-safe [`ChatProvider`] trait so callers
//! (the agent loop, mocked/replay tests) depend on an abstraction rather
//! than the concrete [`crate::Client`]. Transport-only — no agent logic
//! lives here.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;

use crate::client::ProviderError;
use crate::types::{ChatRequest, StreamEvent};

/// Receiver half of a streamed chat completion: one `Ok` per decoded SSE
/// event, terminated by either normal channel closure or a single `Err`.
pub type EventStream = mpsc::Receiver<Result<StreamEvent, ProviderError>>;

/// Object-safe seam between the agent loop and whatever transport serves
/// chat completions (real HTTP client today; recorded/replay fixtures for
/// Task 8). Implementors must be cheaply cloneable behind `Arc` and safe
/// to share across the sub-agent runner and the main loop.
pub trait ChatProvider: Send + Sync {
    /// Start a streaming chat completion; see [`crate::Client::stream_chat`]
    /// for the channel/abort contract.
    fn stream_chat(&self, request: &ChatRequest, abort: Arc<AtomicBool>) -> EventStream;

    /// Replace the API key used on subsequent requests.
    fn set_api_key(&self, key: Option<String>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Client;

    #[test]
    fn client_implements_chat_provider() {
        fn accepts(_: &dyn ChatProvider) {}
        let client = Client::new("http://localhost:1", None).unwrap();
        accepts(&client);
    }
}
