use serde::{Deserialize, Serialize};

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
///
/// Adjacently tagged so every variant shape (newtype over a string, over
/// a struct, over an enum, and a struct variant) round-trips through a
/// cassette unchanged; an internally tagged form cannot carry the
/// newtype-over-string variants at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum StreamEvent {
    TextDelta(String),
    ReasoningDelta(String),
    #[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Other(String),
}

impl FinishReason {
    pub(super) fn from_wire(s: &str) -> Self {
        match s {
            "stop" => FinishReason::Stop,
            "tool_calls" | "function_call" => FinishReason::ToolCalls,
            "length" => FinishReason::Length,
            "content_filter" => FinishReason::ContentFilter,
            other => FinishReason::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replay serves recorded events verbatim, so every variant shape has
    /// to survive the trip through a cassette line.
    #[test]
    fn every_stream_event_round_trips_through_json() {
        let events = vec![
            StreamEvent::TextDelta("hello".into()),
            StreamEvent::ReasoningDelta("thinking".into()),
            StreamEvent::ToolCallDelta {
                index: 0,
                id: Some("call_1".into()),
                name: Some("bash".into()),
                args_delta: "{\"command\":\"ls\"}".into(),
            },
            StreamEvent::Usage(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
            }),
            StreamEvent::Finish(FinishReason::ToolCalls),
            StreamEvent::Finish(FinishReason::Other("weird".into())),
            StreamEvent::Done,
        ];
        let text = serde_json::to_string(&events).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<StreamEvent>>(&text).unwrap(),
            events
        );
    }
}
