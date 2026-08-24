//! Wire types for the OpenAI-compatible Chat Completions API plus the
//! provider-agnostic streaming event surface consumed by the agent loop.

use serde::{Deserialize, Serialize};

/// A chat completion request. Always sent with `stream: true`.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDef>,
    pub stream: bool,
    pub stream_options: StreamOptions,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            stream: true,
            stream_options: StreamOptions { include_usage: true },
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolDef>) -> Self {
        self.tools = tools;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct StreamOptions {
    include_usage: bool,
}

/// A conversation message, tagged by role.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        ChatMessage::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        ChatMessage::User {
            content: content.into(),
        }
    }

    pub fn assistant_text(content: impl Into<String>) -> Self {
        ChatMessage::Assistant {
            content: Some(content.into()),
            tool_calls: Vec::new(),
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ChatMessage::Tool {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }
}

/// A tool call emitted by the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded argument object (a string on the wire, per spec).
    pub arguments: String,
}

/// Tool definition advertised to the model.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunctionDef,
}

impl ToolDef {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            kind: "function".to_string(),
            function: ToolFunctionDef {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Token usage reported by the provider (authoritative budget signal).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }
}

/// Parsed streaming event — everything downstream ever sees.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    TextDelta(String),
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        args_delta: String,
    },
    Usage(Usage),
    Finish(FinishReason),
    /// SSE `[DONE]` sentinel received.
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Other(String),
}

impl FinishReason {
    fn from_wire(s: &str) -> Self {
        match s {
            "stop" => FinishReason::Stop,
            "tool_calls" | "function_call" => FinishReason::ToolCalls,
            "length" => FinishReason::Length,
            "content_filter" => FinishReason::ContentFilter,
            other => FinishReason::Other(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire (deserialization) shapes for SSE chunks
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct SseChunk {
    #[serde(default)]
    choices: Vec<SseChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SseChoice {
    #[serde(default)]
    delta: Option<SseDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SseDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<SseToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SseToolCallDelta {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<SseFunctionDelta>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SseFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Convert one SSE `data:` payload into zero or more stream events.
///
/// Lenient by design: unknown fields are ignored; a payload that fails to
/// parse yields an error the caller may log-and-skip (transport-level
/// resilience), while *tool-argument* malformation is handled later by the
/// accumulator so the model can self-correct.
pub(crate) fn parse_chunk_data(data: &str) -> Result<Vec<StreamEvent>, serde_json::Error> {
    let data = data.trim();
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if data == "[DONE]" {
        return Ok(vec![StreamEvent::Done]);
    }
    let chunk: SseChunk = serde_json::from_str(data)?;
    let mut events = Vec::new();

    // Usage before Finish: consumers may stop at the finish marker, and
    // both often arrive in the same final chunk.
    if let Some(usage) = chunk.usage {
        if usage.prompt_tokens != 0 || usage.completion_tokens != 0 {
            events.push(StreamEvent::Usage(usage));
        }
    }

    for choice in &chunk.choices {
        if let Some(delta) = &choice.delta {
            if let Some(text) = &delta.content {
                if !text.is_empty() {
                    events.push(StreamEvent::TextDelta(text.clone()));
                }
            }
            if let Some(calls) = &delta.tool_calls {
                for call in calls {
                    events.push(StreamEvent::ToolCallDelta {
                        index: call.index,
                        id: call.id.clone(),
                        name: call.function.as_ref().and_then(|f| f.name.clone()),
                        args_delta: call
                            .function
                            .as_ref()
                            .and_then(|f| f.arguments.clone())
                            .unwrap_or_default(),
                    });
                }
            }
        }
        if let Some(reason) = &choice.finish_reason {
            events.push(StreamEvent::Finish(FinishReason::from_wire(reason)));
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_delta() {
        let ev = parse_chunk_data(
            r#"{"choices":[{"delta":{"content":"Hi"},"index":0}]}"#,
        )
        .unwrap();
        assert_eq!(ev, vec![StreamEvent::TextDelta("Hi".into())]);
    }

    #[test]
    fn parses_done_sentinel() {
        assert_eq!(parse_chunk_data("[DONE]").unwrap(), vec![StreamEvent::Done]);
    }

    #[test]
    fn empty_and_whitespace_payloads_yield_no_events() {
        assert!(parse_chunk_data("").unwrap().is_empty());
        assert!(parse_chunk_data("   ").unwrap().is_empty());
    }

    #[test]
    fn parses_tool_call_delta_with_partial_fields() {
        let ev = parse_chunk_data(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"rea","arguments":"{\"pa"}}]}}]}"#,
        )
        .unwrap();
        assert_eq!(
            ev,
            vec![StreamEvent::ToolCallDelta {
                index: 0,
                id: Some("call_1".into()),
                name: Some("rea".into()),
                args_delta: "{\"pa".into(),
            }]
        );
    }

    #[test]
    fn parses_finish_reason_and_usage() {
        let ev = parse_chunk_data(
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
        )
        .unwrap();
        assert!(ev.contains(&StreamEvent::Finish(FinishReason::ToolCalls)));
        assert!(ev.contains(&StreamEvent::Usage(Usage { prompt_tokens: 10, completion_tokens: 5 })));
    }

    #[test]
    fn usage_only_final_chunk_has_no_choices() {
        let ev = parse_chunk_data(r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":9}}"#).unwrap();
        assert_eq!(ev, vec![StreamEvent::Usage(Usage { prompt_tokens: 3, completion_tokens: 9 })]);
    }
}
