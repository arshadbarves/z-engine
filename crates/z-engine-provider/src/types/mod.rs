//! Wire types for the OpenAI-compatible Chat Completions API plus the
//! provider-agnostic streaming event surface consumed by the agent loop.

mod request;
mod stream;
mod tools;
mod wire;

pub use request::{
    ChatMessage, ChatRequest, ContentPart, ImageUrlBody, ReasoningParam, StreamOptions,
};
pub use stream::{FinishReason, StreamEvent, Usage};
pub use tools::{FunctionCall, ToolCall, ToolDef, ToolFunctionDef};

pub(crate) use wire::parse_chunk_data;
