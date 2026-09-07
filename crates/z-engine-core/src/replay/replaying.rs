//! [`ReplayProvider`]: the cassette as a transport.
//!
//! It holds no HTTP client, no base URL, and no API key — there is
//! nothing here that *could* reach the network. Requests are matched
//! against the recorded sequence by the hash of their exact serialized
//! bytes, in order. The first request that does not match ends the run
//! with [`ProviderError::Replay`] carrying the sequence it diverged at,
//! and the divergence is kept as a typed [`ReplayMismatch`] for the
//! caller to inspect.
//!
//! There is deliberately no "close enough" path and no fallback: a
//! replay that improvised would be a new run wearing the old one's name.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use z_engine_provider::{ChatProvider, ChatRequest, EventStream, ProviderError};

use super::entry::{ProviderExchange, canonical_request};
use super::tape::RunCassette;

/// Where and how a replay stopped being the run it was replaying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayMismatch {
    /// Position in the recorded request sequence (0-based).
    pub sequence: u64,
    /// Hash of the request recorded at that position; empty when the
    /// cassette holds no request there at all.
    pub expected_hash: String,
    /// Hash of the request this run actually made.
    pub actual_hash: String,
    /// What diverged, in words.
    pub detail: String,
    /// The recorded request, serialized, for diffing against `actual`.
    pub expected: Option<String>,
    /// The request this run actually made, serialized.
    pub actual: String,
}

/// Serves one recorded run back, in order, and nothing else.
#[derive(Debug)]
pub struct ReplayProvider {
    exchanges: Vec<ProviderExchange>,
    cursor: AtomicUsize,
    mismatch: Mutex<Option<ReplayMismatch>>,
}

impl ReplayProvider {
    pub fn new(cassette: &RunCassette) -> Self {
        Self {
            exchanges: cassette.exchanges(),
            cursor: AtomicUsize::new(0),
            mismatch: Mutex::new(None),
        }
    }

    /// The first divergence, if this replay diverged.
    pub fn mismatch(&self) -> Option<ReplayMismatch> {
        self.mismatch.lock().expect("replay mismatch").clone()
    }

    /// How many requests the cassette holds.
    pub fn len(&self) -> usize {
        self.exchanges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.exchanges.is_empty()
    }

    /// Requests served so far.
    pub fn served(&self) -> usize {
        self.cursor.load(Ordering::SeqCst).min(self.exchanges.len())
    }

    /// Keep the *first* divergence: later requests are downstream of it
    /// and would only bury the cause.
    fn remember(&self, mismatch: ReplayMismatch) {
        let mut slot = self.mismatch.lock().expect("replay mismatch");
        if slot.is_none() {
            *slot = Some(mismatch);
        }
    }
}

/// One terminal error, delivered on its own channel.
fn refuse(sequence: u64, detail: String) -> EventStream {
    let (tx, rx) = mpsc::channel(1);
    let _ = tx.try_send(Err(ProviderError::Replay { sequence, detail }));
    rx
}

impl ChatProvider for ReplayProvider {
    fn stream_chat(&self, request: &ChatRequest, _abort: Arc<AtomicBool>) -> EventStream {
        let index = self.cursor.fetch_add(1, Ordering::SeqCst);
        let sequence = index as u64;
        let (actual, actual_hash) = match canonical_request(request) {
            Ok(pair) => pair,
            Err(err) => {
                let detail = format!("request could not be serialized for matching: {err}");
                self.remember(ReplayMismatch {
                    sequence,
                    expected_hash: String::new(),
                    actual_hash: String::new(),
                    detail: detail.clone(),
                    expected: None,
                    actual: String::new(),
                });
                return refuse(sequence, detail);
            }
        };

        let Some(exchange) = self.exchanges.get(index) else {
            let detail = format!(
                "the cassette holds {} request(s); this run made at least {}",
                self.exchanges.len(),
                index + 1
            );
            self.remember(ReplayMismatch {
                sequence,
                expected_hash: String::new(),
                actual_hash,
                detail: detail.clone(),
                expected: None,
                actual,
            });
            return refuse(sequence, detail);
        };

        if exchange.request_hash != actual_hash {
            let expected = canonical_request(&exchange.request)
                .map(|(text, _)| text)
                .ok();
            let detail = format!(
                "request does not match the recording (expected {}, got {})",
                &exchange.request_hash, &actual_hash
            );
            self.remember(ReplayMismatch {
                sequence: exchange.sequence,
                expected_hash: exchange.request_hash.clone(),
                actual_hash,
                detail: detail.clone(),
                expected,
                actual,
            });
            return refuse(exchange.sequence, detail);
        }

        // Capacity covers the whole recorded stream, so serving it needs
        // no task and no scheduling decisions of its own.
        let (tx, rx) = mpsc::channel(exchange.events.len() + 1);
        for event in &exchange.events {
            let _ = tx.try_send(Ok(event.clone()));
        }
        if let Some(error) = &exchange.error {
            let _ = tx.try_send(Err(ProviderError::StreamInterrupted(format!(
                "recorded provider failure: {error}"
            ))));
        }
        rx
    }

    /// A cassette has no credentials to set; taking one would suggest it
    /// might use them.
    fn set_api_key(&self, _key: Option<String>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::recorder::RunRecorder;
    use z_engine_provider::{ChatMessage, FinishReason, StreamEvent};

    fn taped(dir: &tempfile::TempDir, requests: &[(&str, Vec<StreamEvent>)]) -> RunCassette {
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        for (prompt, events) in requests {
            let request = ChatRequest::new("m", vec![ChatMessage::user(*prompt)]);
            let (_, hash) = canonical_request(&request).unwrap();
            let seq = recorder.begin_exchange();
            recorder.finish_exchange(seq, &request, &hash, events, None);
        }
        RunCassette::load(&path).unwrap()
    }

    async fn drain(mut rx: EventStream) -> Vec<Result<StreamEvent, ProviderError>> {
        let mut out = Vec::new();
        while let Some(item) = rx.recv().await {
            out.push(item);
        }
        out
    }

    #[tokio::test]
    async fn matching_requests_are_served_in_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(
            &dir,
            &[
                ("one", vec![StreamEvent::TextDelta("first".into())]),
                ("two", vec![StreamEvent::Finish(FinishReason::Stop)]),
            ],
        );
        let replay = ReplayProvider::new(&cassette);
        let abort = Arc::new(AtomicBool::new(false));

        let first = drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("one")]),
            Arc::clone(&abort),
        ))
        .await;
        assert!(
            matches!(first.as_slice(), [Ok(StreamEvent::TextDelta(t))] if t == "first"),
            "{first:?}"
        );
        let second = drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("two")]),
            abort,
        ))
        .await;
        assert!(
            matches!(
                second.as_slice(),
                [Ok(StreamEvent::Finish(FinishReason::Stop))]
            ),
            "{second:?}"
        );
        assert!(replay.mismatch().is_none());
        assert_eq!(replay.served(), 2);
    }

    #[tokio::test]
    async fn the_first_divergence_fails_with_its_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(
            &dir,
            &[
                ("one", vec![StreamEvent::Done]),
                ("two", vec![StreamEvent::Done]),
            ],
        );
        let replay = ReplayProvider::new(&cassette);
        let abort = Arc::new(AtomicBool::new(false));
        drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("one")]),
            Arc::clone(&abort),
        ))
        .await;

        let out = drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("elsewhere")]),
            Arc::clone(&abort),
        ))
        .await;
        assert!(
            matches!(
                out.as_slice(),
                [Err(ProviderError::Replay { sequence: 1, .. })]
            ),
            "{out:?}"
        );
        let mismatch = replay.mismatch().unwrap();
        assert_eq!(mismatch.sequence, 1);
        assert_ne!(mismatch.expected_hash, mismatch.actual_hash);
        assert!(mismatch.expected.unwrap().contains("two"));

        // A later request must not overwrite the cause.
        drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("later still")]),
            abort,
        ))
        .await;
        assert_eq!(replay.mismatch().unwrap().sequence, 1);
    }

    #[tokio::test]
    async fn running_past_the_end_of_the_tape_is_a_refusal_not_a_silence() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(&dir, &[("one", vec![StreamEvent::Done])]);
        let replay = ReplayProvider::new(&cassette);
        let abort = Arc::new(AtomicBool::new(false));
        drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("one")]),
            Arc::clone(&abort),
        ))
        .await;

        let out = drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("one")]),
            abort,
        ))
        .await;
        assert!(
            matches!(
                out.as_slice(),
                [Err(ProviderError::Replay { sequence: 1, .. })]
            ),
            "{out:?}"
        );
        assert!(replay.mismatch().unwrap().detail.contains("holds 1"));
    }

    #[tokio::test]
    async fn a_recorded_failure_replays_as_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        let request = ChatRequest::new("m", vec![ChatMessage::user("one")]);
        let (_, hash) = canonical_request(&request).unwrap();
        let seq = recorder.begin_exchange();
        recorder.finish_exchange(
            seq,
            &request,
            &hash,
            &[StreamEvent::TextDelta("par".into())],
            Some("upstream exploded".into()),
        );
        let cassette = RunCassette::load(&path).unwrap();

        let replay = ReplayProvider::new(&cassette);
        let out = drain(replay.stream_chat(&request, Arc::new(AtomicBool::new(false)))).await;
        assert_eq!(out.len(), 2);
        assert!(matches!(
            out[1],
            Err(ProviderError::StreamInterrupted(ref d)) if d.contains("upstream exploded")
        ));
    }
}
