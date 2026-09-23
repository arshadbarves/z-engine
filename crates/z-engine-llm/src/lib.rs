//! Model transport: the [`ModelClient`] seam, provider adapters, retries,
//! fallback models, the model catalog and cost accounting.

pub mod accumulate;
pub mod client;
pub mod error;
pub mod types;

pub use accumulate::{AssistantTurn, MalformedToolUse, ResponseAccumulator};
pub use client::{ModelClient, collect};
pub use error::LlmError;
pub use types::{
    ModelEvent, ModelRequest, ModelStream, StopReason, SystemBlock, ThinkingConfig, ToolChoice,
    ToolSpec,
};
