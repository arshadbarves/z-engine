//! The provider seam: an object-safe [`ChatProvider`] trait so callers
//! (the agent loop, mocked/replay tests) depend on an abstraction rather
//! than the concrete [`crate::Client`]. Transport-only — no agent logic
//! lives here.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;

use crate::client::ProviderError;
use crate::lane::RequestLane;
use crate::types::{ChatRequest, StreamEvent};

/// Receiver half of a streamed chat completion: one `Ok` per decoded SSE
/// event, terminated by either normal channel closure or a single `Err`.
pub type EventStream = mpsc::Receiver<Result<StreamEvent, ProviderError>>;

/// Object-safe seam between the agent loop and whatever transport serves
/// chat completions (real HTTP client today; recorded/replay fixtures for
/// Task 8). Implementors must be cheaply cloneable behind `Arc` and safe
/// to share across the sub-agent runner and the main loop.
///
/// One handle serves many concurrent callers, so every call names the
/// logical stream it belongs to ([`RequestLane`]). A plain transport may
/// ignore the lane entirely; one that records or reproduces traffic
/// cannot, because arrival order across independent callers is the
/// scheduler's business rather than the run's.
pub trait ChatProvider: Send + Sync {
    /// Start a streaming chat completion on `lane`; see
    /// [`crate::Client::stream_chat`] for the channel/abort contract.
    fn stream_chat_on(
        &self,
        lane: &RequestLane,
        request: &ChatRequest,
        abort: Arc<AtomicBool>,
    ) -> EventStream;

    /// Start a streaming chat completion on [`RequestLane::MAIN`].
    fn stream_chat(&self, request: &ChatRequest, abort: Arc<AtomicBool>) -> EventStream {
        self.stream_chat_on(&RequestLane::MAIN, request, abort)
    }

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

    /// A transport with no interest in lanes still has to be reachable
    /// through the plain call, and that call must mean the main lane.
    #[tokio::test]
    async fn the_plain_call_is_the_main_lane() {
        struct Noting(std::sync::Mutex<Vec<RequestLane>>);
        impl ChatProvider for Noting {
            fn stream_chat_on(
                &self,
                lane: &RequestLane,
                _request: &ChatRequest,
                _abort: Arc<AtomicBool>,
            ) -> EventStream {
                self.0.lock().unwrap().push(lane.clone());
                mpsc::channel(1).1
            }
            fn set_api_key(&self, _key: Option<String>) {}
        }

        let provider = Noting(std::sync::Mutex::new(Vec::new()));
        let request = ChatRequest::new("m", vec![crate::ChatMessage::user("hi")]);
        provider.stream_chat(&request, Arc::new(AtomicBool::new(false)));
        provider.stream_chat_on(
            &RequestLane::named("title"),
            &request,
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(
            *provider.0.lock().unwrap(),
            [RequestLane::MAIN, RequestLane::named("title")]
        );
    }
}
