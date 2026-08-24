//! Incremental `text/event-stream` parsing.
//!
//! Spec-correct framing: events are separated by blank lines, multiple
//! `data:` lines inside one event join with `\n`, `:`-prefixed lines are
//! comments (OpenRouter sends them as processing keep-alives), and `\r\n`
//! is tolerated. Feeding may split anywhere — including mid-UTF-8-char.

use super::types::{self, StreamEvent};
use std::collections::VecDeque;

/// Byte-level SSE framer: yields the `data:` payload of each completed event.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event_data: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed arbitrary network bytes; returns payloads of completed events.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
            let mut line =
                String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1]).into_owned();
            if line.ends_with('\r') {
                line.pop();
            }
            self.process_line(&line, &mut out);
        }
        out
    }

    /// Flush any pending partial line / undelivered event (call at stream end).
    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.buf.is_empty() {
            let rest = String::from_utf8_lossy(&self.buf).into_owned();
            self.buf.clear();
            self.process_line(&rest, &mut out);
        }
        if !self.event_data.is_empty() {
            out.push(self.event_data.join("\n"));
            self.event_data.clear();
        }
        out
    }

    fn process_line(&mut self, line: &str, out: &mut Vec<String>) {
        if line.is_empty() {
            if !self.event_data.is_empty() {
                out.push(self.event_data.join("\n"));
                self.event_data.clear();
            }
            return;
        }
        if line.starts_with(':') {
            return; // comment / keep-alive
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let value = rest.strip_prefix(' ').unwrap_or(rest);
            self.event_data.push(value.to_string());
        }
        // `event:`, `id:`, `retry:` fields are irrelevant here — ignored.
    }
}

/// High-level decoder: bytes in, typed [`StreamEvent`]s out.
///
/// Transport-level resilience: an unparseable *chunk* is skipped with a
/// warning (never crashes the stream). Malformed **tool-call arguments**
/// surface later, via the accumulator, as an error tool-result so the model
/// can retry (spec §10).
#[derive(Debug, Default)]
pub struct SseDecoder {
    parser: SseParser,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, bytes: &[u8]) -> VecDeque<StreamEvent> {
        let mut out = VecDeque::new();
        for payload in self.parser.feed(bytes) {
            self.decode_payload(&payload, &mut out);
        }
        out
    }

    pub fn finish(&mut self) -> VecDeque<StreamEvent> {
        let mut out = VecDeque::new();
        for payload in self.parser.finish() {
            self.decode_payload(&payload, &mut out);
        }
        out
    }

    fn decode_payload(&mut self, payload: &str, out: &mut VecDeque<StreamEvent>) {
        match types::parse_chunk_data(payload) {
            Ok(events) => out.extend(events),
            Err(e) => {
                tracing::warn!(%e, len = payload.len(), "skipping unparseable SSE chunk");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT_SSE: &str = include_str!("../../tests/fixtures/sse/text.sse");
    const TOOL_SINGLE_SSE: &str = include_str!("../../tests/fixtures/sse/tool_single.sse");
    const TOOL_MULTI_SSE: &str =
        include_str!("../../tests/fixtures/sse/tool_multi_interleaved.sse");
    const USAGE_FINAL_SSE: &str = include_str!("../../tests/fixtures/sse/usage_final.sse");
    const KEEPALIVE_SSE: &str = include_str!("../../tests/fixtures/sse/keepalive_comments.sse");

    fn decode_all(input: &str) -> Vec<StreamEvent> {
        let mut dec = SseDecoder::new();
        let mut ev = dec.feed(input.as_bytes());
        ev.extend(dec.finish());
        ev.into_iter().collect()
    }

    /// Same as [`decode_all`] but delivered one byte at a time.
    fn decode_bytewise(input: &str) -> Vec<StreamEvent> {
        let mut dec = SseDecoder::new();
        let mut ev = VecDeque::new();
        for b in input.as_bytes() {
            ev.extend(dec.feed(std::slice::from_ref(b)));
        }
        ev.extend(dec.finish());
        ev.into_iter().collect()
    }

    #[test]
    fn text_fixture_yields_deltas_then_done() {
        let ev = decode_all(TEXT_SSE);
        assert_eq!(
            ev,
            vec![
                StreamEvent::TextDelta("Hello".into()),
                StreamEvent::TextDelta(", world".into()),
                // usage precedes finish within the final chunk by design
                StreamEvent::Usage(types::Usage {
                    prompt_tokens: 4,
                    completion_tokens: 3
                }),
                StreamEvent::Finish(FinishReasonEnum::Stop),
                StreamEvent::Done,
            ]
        );
    }

    #[test]
    fn bytewise_feeding_matches_wholesale_feeding() {
        assert_eq!(decode_bytewise(TEXT_SSE), decode_all(TEXT_SSE));
        assert_eq!(decode_bytewise(TOOL_MULTI_SSE), decode_all(TOOL_MULTI_SSE));
        assert_eq!(decode_bytewise(KEEPALIVE_SSE), decode_all(KEEPALIVE_SSE));
    }

    #[test]
    fn tool_call_fixture_streams_id_name_and_args() {
        let ev = decode_all(TOOL_SINGLE_SSE);
        assert!(ev.contains(&StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("call_abc".into()),
            name: Some("read_file".into()),
            args_delta: String::new(),
        }));
        let joined: String = ev
            .iter()
            .filter_map(|e| match e {
                StreamEvent::ToolCallDelta { args_delta, .. } => Some(args_delta.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(joined, r#"{"path": "src/main.rs"}"#);
        assert!(ev.contains(&StreamEvent::Finish(FinishReasonEnum::ToolCalls)));
    }

    #[test]
    fn interleaved_multi_tool_fixture_keeps_indexes() {
        let ev = decode_all(TOOL_MULTI_SSE);
        let calls: Vec<(usize, Option<&str>)> = ev
            .iter()
            .filter_map(|e| match e {
                StreamEvent::ToolCallDelta { index, name, .. } => Some((*index, name.as_deref())),
                _ => None,
            })
            .collect();
        assert_eq!(
            calls,
            vec![(0, Some("read_file")), (1, Some("bash")), (0, None)]
        );
    }

    #[test]
    fn usage_only_final_chunk_is_surfaced() {
        let ev = decode_all(USAGE_FINAL_SSE);
        assert!(ev.contains(&StreamEvent::Usage(types::Usage {
            prompt_tokens: 120,
            completion_tokens: 45
        })));
        assert_eq!(*ev.last().unwrap(), StreamEvent::Done);
    }

    #[test]
    fn keepalive_comments_are_ignored() {
        let ev = decode_all(KEEPALIVE_SSE);
        assert_eq!(
            ev,
            vec![
                StreamEvent::TextDelta("ok".into()),
                StreamEvent::Finish(FinishReasonEnum::Stop),
                StreamEvent::Done,
            ]
        );
    }

    #[test]
    fn crlf_line_endings_tolerated() {
        let crlf = TEXT_SSE.replace('\n', "\r\n");
        assert_eq!(decode_all(&crlf), decode_all(TEXT_SSE));
    }

    #[test]
    fn garbage_chunks_are_skipped_not_fatal() {
        let mixed = format!("data: not-json\n\ndata: [DONE]\n\n{TEXT_SSE}");
        let ev = decode_all(&mixed);
        assert!(ev.contains(&StreamEvent::Done));
        assert!(ev.contains(&StreamEvent::TextDelta("Hello".into())));
    }

    // alias to keep import list tidy in assertions above
    use types::FinishReason as FinishReasonEnum;
}
