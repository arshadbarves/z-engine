//! OpenAI-compatible provider access: wire types, SSE decoding, tool-call
//! delta accumulation, and the HTTP client.

pub mod accumulate;
pub mod client;
pub mod sse;
pub mod types;

pub use accumulate::{AccumulatedToolCall, ToolCallAccumulator};
pub use client::{Client, ProviderError};
pub use types::{
    ChatMessage, ChatRequest, ContentPart, FinishReason, FunctionCall, StreamEvent, ToolCall,
    ToolDef, ToolFunctionDef, Usage,
};

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT_SSE: &str = include_str!("../tests/fixtures/sse/text.sse");

    /// End-to-end through decoder: request → events for the canonical
    /// text fixture (integration-level sanity at unit cost).
    #[test]
    fn decoder_pipeline_on_text_fixture() {
        let mut dec = sse::SseDecoder::new();
        let events: Vec<StreamEvent> = dec
            .feed(TEXT_SSE.as_bytes())
            .into_iter()
            .chain(dec.finish())
            .collect();
        assert_eq!(events.len(), 5);
        assert!(matches!(events.last(), Some(StreamEvent::Done)));
    }

    #[test]
    fn chat_message_serialization_shapes() {
        let msgs = vec![
            ChatMessage::system("be terse"),
            ChatMessage::user("hi"),
            ChatMessage::Assistant {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    function: FunctionCall {
                        name: "bash".into(),
                        arguments: "{}".into(),
                    },
                }],
            },
            ChatMessage::tool_result("call_1", "ok"),
        ];
        let json = serde_json::to_string(&msgs).unwrap();
        assert!(json.contains(r#""role":"assistant""#));
        assert!(json.contains(r#""tool_calls""#));
        assert!(json.contains(r#""role":"tool""#));
        assert!(!json.contains(r#""content":null"#)); // omitted, not null
    }
}
