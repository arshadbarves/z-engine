//! The native Anthropic Messages [`ModelClient`].

use reqwest::header::{HeaderMap, HeaderValue};
use tokio_util::sync::CancellationToken;

use super::request::{build_body, thinking_enabled};
use super::stream::MessagesStreamParser;
use crate::client::ModelClient;
use crate::transport::Endpoint;
use crate::types::{ModelRequest, ModelStream};

/// Lets the model think between tool calls within one response.
const INTERLEAVED_THINKING_BETA: &str = "interleaved-thinking-2025-05-14";

#[derive(Debug, Clone)]
pub(crate) struct AnthropicClient {
    endpoint: Endpoint,
    cache_control: bool,
}

impl AnthropicClient {
    pub(crate) fn new(endpoint: Endpoint, cache_control: bool) -> Self {
        Self {
            endpoint,
            cache_control,
        }
    }
}

impl ModelClient for AnthropicClient {
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream {
        let body = build_body(&request, self.cache_control);
        let mut headers = HeaderMap::new();
        if thinking_enabled(&request) && !request.tools.is_empty() {
            headers.insert(
                "anthropic-beta",
                HeaderValue::from_static(INTERLEAVED_THINKING_BETA),
            );
        }
        self.endpoint.stream(
            &body,
            headers,
            request.session_key.as_deref(),
            MessagesStreamParser::default(),
            cancel,
        )
    }

    fn provider(&self) -> &str {
        self.endpoint.label
    }
}
