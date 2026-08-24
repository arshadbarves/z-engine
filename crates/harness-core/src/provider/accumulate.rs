//! Merges streamed tool-call deltas into complete [`ToolCall`]s.
//!
//! Deltas arrive keyed by `index`; `id` and function `name` are set once
//! (usually on the first delta) while `arguments` fragments concatenate.
//! On finalize each call is parsed independently — a call whose arguments
//! never form valid JSON becomes an error entry (returned to the model as
//! an error tool-result by the agent loop), never a loop crash.

use super::types::{FunctionCall, ToolCall};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Default)]
pub struct ToolCallAccumulator {
    partial: BTreeMap<usize, PartialToolCall>,
}

/// Result of finalizing one accumulated call.
#[derive(Debug)]
pub enum AccumulatedToolCall {
    Complete(ToolCall),
    /// Arguments were not valid JSON; carries everything needed to build an
    /// error tool-result addressed to the same `id`.
    MalformedArguments {
        id: String,
        name: Option<String>,
        raw_arguments: String,
        reason: String,
    },
    /// A delta stream existed for this index but no id was ever provided.
    MissingId { index: usize },
}

impl ToolCallAccumulator {
    pub fn absorb(
        &mut self,
        index: usize,
        id: Option<&str>,
        name: Option<&str>,
        args_delta: &str,
    ) {
        let p = self.partial.entry(index).or_default();
        if let Some(id) = id {
            if p.id.is_none() {
                p.id = Some(id.to_string());
            }
        }
        if let Some(name) = name {
            if p.name.is_none() {
                p.name = Some(name.to_string());
            }
        }
        p.arguments.push_str(args_delta);
    }

    pub fn is_empty(&self) -> bool {
        self.partial.is_empty()
    }

    /// Finalize in index order. Empty/whitespace argument strings are
    /// normalized to `{}` (providers omit arguments for zero-param calls).
    pub fn finish(self) -> Vec<AccumulatedToolCall> {
        self.partial
            .into_iter()
            .map(|(index, p)| match p.id {
                None => AccumulatedToolCall::MissingId { index },
                Some(id) => {
                    let raw = p.arguments.trim();
                    let normalized = if raw.is_empty() { "{}" } else { raw };
                    match serde_json::from_str::<serde_json::Value>(normalized) {
                        Ok(v @ serde_json::Value::Object(_)) => {
                            AccumulatedToolCall::Complete(ToolCall {
                                id,
                                function: FunctionCall {
                                    name: p.name.unwrap_or_default(),
                                    arguments: v.to_string(),
                                },
                            })
                        }
                        Ok(_) => AccumulatedToolCall::MalformedArguments {
                            id,
                            name: p.name,
                            reason: "arguments must be a JSON object".to_string(),
                            raw_arguments: normalized.to_string(),
                        },
                        Err(e) => AccumulatedToolCall::MalformedArguments {
                            id,
                            name: p.name,
                            reason: e.to_string(),
                            raw_arguments: normalized.to_string(),
                        },
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::sse::SseDecoder;
    use crate::provider::types::StreamEvent;

    fn accumulate_stream(sse: &str) -> Vec<AccumulatedToolCall> {
        let mut dec = SseDecoder::new();
        let events: Vec<_> = dec.feed(sse.as_bytes()).into_iter().chain(dec.finish()).collect();
        let mut acc = ToolCallAccumulator::default();
        for ev in events {
            if let StreamEvent::ToolCallDelta {
                index,
                id,
                name,
                args_delta,
            } = ev
            {
                acc.absorb(index, id.as_deref(), name.as_deref(), &args_delta);
            }
        }
        acc.finish()
    }

    const TOOL_SINGLE_SSE: &str = include_str!("../../tests/fixtures/sse/tool_single.sse");
    const TOOL_MULTI_SSE: &str =
        include_str!("../../tests/fixtures/sse/tool_multi_interleaved.sse");
    const MALFORMED_SSE: &str = include_str!("../../tests/fixtures/sse/malformed_tool_json.sse");

    #[test]
    fn single_call_assembles_from_fragments() {
        let out = accumulate_stream(TOOL_SINGLE_SSE);
        assert_eq!(out.len(), 1);
        match &out[0] {
            AccumulatedToolCall::Complete(call) => {
                assert_eq!(call.id, "call_abc");
                assert_eq!(call.function.name, "read_file");
                let args: serde_json::Value =
                    serde_json::from_str(&call.function.arguments).unwrap();
                assert_eq!(args["path"], "src/main.rs");
            }
            other => panic!("expected complete call, got {other:?}"),
        }
    }

    #[test]
    fn interleaved_calls_stay_separate_and_ordered() {
        let out = accumulate_stream(TOOL_MULTI_SSE);
        assert_eq!(out.len(), 2);
        let names: Vec<Option<&str>> = out
            .iter()
            .map(|c| match c {
                AccumulatedToolCall::Complete(c) => Some(c.function.name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec![Some("read_file"), Some("bash")]);
        // bash args were delivered whole on index 1 before read_file's tail
        match &out[1] {
            AccumulatedToolCall::Complete(c) => {
                let args: serde_json::Value =
                    serde_json::from_str(&c.function.arguments).unwrap();
                assert_eq!(args["command"], "pwd");
            }
            other => panic!("expected complete bash call, got {other:?}"),
        }
    }

    #[test]
    fn malformed_arguments_become_error_entry_not_panic() {
        let out = accumulate_stream(MALFORMED_SSE);
        assert_eq!(out.len(), 1);
        match &out[0] {
            AccumulatedToolCall::MalformedArguments {
                id,
                name,
                raw_arguments,
                ..
            } => {
                assert_eq!(id, "call_bad");
                assert_eq!(name.as_deref(), Some("read_file"));
                assert_eq!(raw_arguments, r#"{ "path": "x.txt""#);
            }
            other => panic!("expected malformed-arguments entry, got {other:?}"),
        }
    }

    #[test]
    fn empty_arguments_normalize_to_empty_object() {
        let mut acc = ToolCallAccumulator::default();
        acc.absorb(0, Some("call_x"), Some("no_args"), "");
        let out = acc.finish();
        match &out[0] {
            AccumulatedToolCall::Complete(c) => {
                assert_eq!(c.function.arguments, "{}");
            }
            other => panic!("expected complete call, got {other:?}"),
        }
    }

    #[test]
    fn non_object_arguments_are_rejected() {
        let mut acc = ToolCallAccumulator::default();
        acc.absorb(0, Some("call_y"), Some("t"), "[1,2]");
        match &acc.finish()[0] {
            AccumulatedToolCall::MalformedArguments { reason, .. } => {
                assert!(reason.contains("object"));
            }
            other => panic!("expected malformed entry, got {other:?}"),
        }
    }

    #[test]
    fn missing_id_is_reported_with_index() {
        let mut acc = ToolCallAccumulator::default();
        acc.absorb(3, None, Some("t"), "{}");
        match &acc.finish()[0] {
            AccumulatedToolCall::MissingId { index } => assert_eq!(*index, 3),
            other => panic!("expected missing-id, got {other:?}"),
        }
    }
}
