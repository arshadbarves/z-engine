//! Native Anthropic Messages API adapter.

mod client;
mod messages;
mod request;
mod stream;

pub(crate) use client::AnthropicClient;
