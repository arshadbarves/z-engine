// ---------------------------------------------------------------------------
// Wire (deserialization) shapes for SSE chunks
// ---------------------------------------------------------------------------

use serde::Deserialize;

use super::stream::{FinishReason, StreamEvent, Usage};

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
    /// Reasoning/thinking tokens (OpenRouter `reasoning_content`, some
    /// providers `reasoning`).
    #[serde(default, alias = "reasoning")]
    reasoning_content: Option<String>,
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
            if let Some(r) = &delta.reasoning_content {
                if !r.is_empty() {
                    events.push(StreamEvent::ReasoningDelta(r.clone()));
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
        let ev = parse_chunk_data(r#"{"choices":[{"delta":{"content":"Hi"},"index":0}]}"#).unwrap();
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
        assert!(ev.contains(&StreamEvent::Usage(Usage {
            prompt_tokens: 10,
            completion_tokens: 5
        })));
    }

    #[test]
    fn usage_only_final_chunk_has_no_choices() {
        let ev =
            parse_chunk_data(r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":9}}"#)
                .unwrap();
        assert_eq!(
            ev,
            vec![StreamEvent::Usage(Usage {
                prompt_tokens: 3,
                completion_tokens: 9
            })]
        );
    }
}
