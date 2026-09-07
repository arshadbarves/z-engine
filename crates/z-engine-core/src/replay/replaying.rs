//! [`ReplayProvider`]: the cassette as a transport.
//!
//! It holds no HTTP client, no base URL, and no API key — there is
//! nothing here that *could* reach the network. Requests are matched
//! against the recorded sequence *for their lane* by the hash of their
//! exact serialized bytes, in order. The first request that does not
//! match ends the run with [`ProviderError::Replay`] carrying the lane
//! and the sequence it diverged at, and the divergence is kept as a
//! typed [`ReplayMismatch`] for the caller to inspect.
//!
//! Matching per lane rather than per tape is what makes a run with side
//! requests replayable at all: the turn loop and the session titler are
//! ordered against themselves and against nothing else.
//!
//! There is deliberately no "close enough" path and no fallback: a
//! replay that improvised would be a new run wearing the old one's name.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use z_engine_provider::{ChatProvider, ChatRequest, EventStream, ProviderError, RequestLane};

use super::entry::canonical_request;
use super::error::ReplayError;
use super::lanes::LaneTable;
use super::mismatch::ReplayMismatch;
use super::tape::RunCassette;

/// Serves one recorded run back, in order, and nothing else.
#[derive(Debug)]
pub struct ReplayProvider {
    table: LaneTable,
    /// One cursor per lane, created on first use.
    cursors: Mutex<HashMap<RequestLane, Arc<AtomicUsize>>>,
    served: AtomicUsize,
    mismatch: Mutex<Option<ReplayMismatch>>,
}

impl ReplayProvider {
    /// Refuses a cassette whose lanes do not count from zero without
    /// gaps — see [`LaneTable::build`].
    pub fn new(cassette: &RunCassette) -> Result<Self, ReplayError> {
        Ok(Self {
            table: LaneTable::build(cassette)?,
            cursors: Mutex::new(HashMap::new()),
            served: AtomicUsize::new(0),
            mismatch: Mutex::new(None),
        })
    }

    /// The first divergence, if this replay diverged.
    pub fn mismatch(&self) -> Option<ReplayMismatch> {
        self.mismatch.lock().expect("replay mismatch").clone()
    }

    /// How many requests the cassette holds, across every lane.
    pub fn len(&self) -> usize {
        self.table.total()
    }

    pub fn is_empty(&self) -> bool {
        self.table.total() == 0
    }

    /// Requests matched and served so far.
    pub fn served(&self) -> usize {
        self.served.load(Ordering::SeqCst)
    }

    fn cursor(&self, lane: &RequestLane) -> Arc<AtomicUsize> {
        Arc::clone(
            self.cursors
                .lock()
                .expect("replay cursors")
                .entry(lane.clone())
                .or_default(),
        )
    }

    /// Keep the *first* divergence: later requests are downstream of it
    /// and would only bury the cause.
    fn remember(&self, mismatch: ReplayMismatch) -> EventStream {
        let refusal = ProviderError::Replay {
            lane: mismatch.lane.clone(),
            sequence: mismatch.sequence,
            detail: mismatch.detail.clone(),
        };
        let mut slot = self.mismatch.lock().expect("replay mismatch");
        if slot.is_none() {
            *slot = Some(mismatch);
        }
        drop(slot);
        // One terminal error, delivered on its own channel.
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.try_send(Err(refusal));
        rx
    }
}

impl ChatProvider for ReplayProvider {
    fn stream_chat_on(
        &self,
        lane: &RequestLane,
        request: &ChatRequest,
        _abort: Arc<AtomicBool>,
    ) -> EventStream {
        let index = self.cursor(lane).fetch_add(1, Ordering::SeqCst);
        // Lanes are contiguous from zero, checked at construction, so
        // the cursor *is* the recorded sequence number whether or not
        // the tape has an entry there.
        let sequence = index as u64;
        let recorded = self.table.lane(lane);

        let (actual, actual_hash) = match canonical_request(request) {
            Ok(pair) => pair,
            Err(err) => {
                return self.remember(ReplayMismatch::unserializable(lane, sequence, err));
            }
        };

        let Some(exchange) = recorded.get(index) else {
            return self.remember(ReplayMismatch::past_the_end(
                lane,
                sequence,
                recorded.len(),
                actual_hash,
                actual,
            ));
        };

        if exchange.request_hash != actual_hash {
            return self.remember(ReplayMismatch::different(
                lane,
                sequence,
                exchange,
                actual_hash,
                actual,
            ));
        }

        self.served.fetch_add(1, Ordering::SeqCst);
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

    fn taped(
        dir: &tempfile::TempDir,
        requests: &[(RequestLane, &str, Vec<StreamEvent>)],
    ) -> RunCassette {
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        for (lane, prompt, events) in requests {
            let request = ChatRequest::new("m", vec![ChatMessage::user(*prompt)]);
            let (_, hash) = canonical_request(&request).unwrap();
            let seq = recorder.begin_exchange(lane);
            recorder.finish_exchange(lane, seq, &request, &hash, events, None);
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

    fn ask(replay: &ReplayProvider, lane: &RequestLane, prompt: &str) -> EventStream {
        replay.stream_chat_on(
            lane,
            &ChatRequest::new("m", vec![ChatMessage::user(prompt)]),
            Arc::new(AtomicBool::new(false)),
        )
    }

    #[tokio::test]
    async fn matching_requests_are_served_in_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(
            &dir,
            &[
                (
                    RequestLane::MAIN,
                    "one",
                    vec![StreamEvent::TextDelta("first".into())],
                ),
                (
                    RequestLane::MAIN,
                    "two",
                    vec![StreamEvent::Finish(FinishReason::Stop)],
                ),
            ],
        );
        let replay = ReplayProvider::new(&cassette).unwrap();

        let first = drain(ask(&replay, &RequestLane::MAIN, "one")).await;
        assert!(
            matches!(first.as_slice(), [Ok(StreamEvent::TextDelta(t))] if t == "first"),
            "{first:?}"
        );
        let second = drain(ask(&replay, &RequestLane::MAIN, "two")).await;
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

    /// A plain `stream_chat` is the main lane, so the turn loop needs no
    /// knowledge of lanes to be replayed.
    #[tokio::test]
    async fn the_default_call_is_matched_against_the_main_lane() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(&dir, &[(RequestLane::MAIN, "one", vec![StreamEvent::Done])]);
        let replay = ReplayProvider::new(&cassette).unwrap();
        let out = drain(replay.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("one")]),
            Arc::new(AtomicBool::new(false)),
        ))
        .await;
        assert!(matches!(out.as_slice(), [Ok(StreamEvent::Done)]), "{out:?}");
    }

    /// Two callers sharing a provider have no order between them, so
    /// each is matched against its own recorded requests — whichever
    /// order they happen to arrive in this time.
    #[tokio::test]
    async fn lanes_are_matched_independently_of_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let title = RequestLane::named("title");
        let cassette = taped(
            &dir,
            &[
                (RequestLane::MAIN, "turn one", vec![StreamEvent::Done]),
                (title.clone(), "name this", vec![StreamEvent::Done]),
                (RequestLane::MAIN, "turn two", vec![StreamEvent::Done]),
            ],
        );

        // Recorded main-then-title-then-main; replayed title first.
        let replay = ReplayProvider::new(&cassette).unwrap();
        for (lane, prompt) in [
            (&title, "name this"),
            (&RequestLane::MAIN, "turn one"),
            (&RequestLane::MAIN, "turn two"),
        ] {
            drain(ask(&replay, lane, prompt)).await;
        }
        assert!(replay.mismatch().is_none(), "{:?}", replay.mismatch());
        assert_eq!(replay.served(), 3);
    }

    /// A lane running out is about that lane, not about the tape's
    /// overall length.
    #[tokio::test]
    async fn a_request_on_an_unrecorded_lane_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(&dir, &[(RequestLane::MAIN, "one", vec![StreamEvent::Done])]);
        let replay = ReplayProvider::new(&cassette).unwrap();
        let out = drain(ask(&replay, &RequestLane::named("title"), "name this")).await;
        assert!(
            matches!(
                out.as_slice(),
                [Err(ProviderError::Replay { lane, sequence: 0, .. })] if lane == "title"
            ),
            "{out:?}"
        );
        assert!(replay.mismatch().unwrap().detail.contains("holds 0"));
    }

    #[tokio::test]
    async fn the_first_divergence_fails_with_its_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(
            &dir,
            &[
                (RequestLane::MAIN, "one", vec![StreamEvent::Done]),
                (RequestLane::MAIN, "two", vec![StreamEvent::Done]),
            ],
        );
        let replay = ReplayProvider::new(&cassette).unwrap();
        drain(ask(&replay, &RequestLane::MAIN, "one")).await;

        let out = drain(ask(&replay, &RequestLane::MAIN, "elsewhere")).await;
        assert!(
            matches!(
                out.as_slice(),
                [Err(ProviderError::Replay { sequence: 1, .. })]
            ),
            "{out:?}"
        );
        let mismatch = replay.mismatch().unwrap();
        assert_eq!(mismatch.sequence, 1);
        assert_eq!(mismatch.lane, "main");
        assert_ne!(mismatch.expected_hash, mismatch.actual_hash);
        assert!(mismatch.expected.unwrap().contains("two"));

        // A later request must not overwrite the cause.
        drain(ask(&replay, &RequestLane::MAIN, "later still")).await;
        assert_eq!(replay.mismatch().unwrap().sequence, 1);
        assert_eq!(replay.served(), 1, "only the matched request was served");
    }

    #[tokio::test]
    async fn running_past_the_end_of_the_tape_is_a_refusal_not_a_silence() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(&dir, &[(RequestLane::MAIN, "one", vec![StreamEvent::Done])]);
        let replay = ReplayProvider::new(&cassette).unwrap();
        drain(ask(&replay, &RequestLane::MAIN, "one")).await;

        let out = drain(ask(&replay, &RequestLane::MAIN, "one")).await;
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
        let seq = recorder.begin_exchange(&RequestLane::MAIN);
        recorder.finish_exchange(
            &RequestLane::MAIN,
            seq,
            &request,
            &hash,
            &[StreamEvent::TextDelta("par".into())],
            Some("upstream exploded".into()),
        );
        let cassette = RunCassette::load(&path).unwrap();

        let replay = ReplayProvider::new(&cassette).unwrap();
        let out = drain(replay.stream_chat(&request, Arc::new(AtomicBool::new(false)))).await;
        assert_eq!(out.len(), 2);
        assert!(matches!(
            out[1],
            Err(ProviderError::StreamInterrupted(ref d)) if d.contains("upstream exploded")
        ));
    }
}
