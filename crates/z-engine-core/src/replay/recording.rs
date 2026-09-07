//! [`RecordingProvider`]: a transparent tap between the agent loop and
//! any [`ChatProvider`].
//!
//! It changes nothing about the request or the stream — it claims the
//! request's position before the call and appends the exchange once the
//! stream ends, forwarding every event to the caller as it arrives so
//! backpressure and abort behaviour are the underlying provider's.
//! Because it wraps the trait rather than the HTTP client, the same tap
//! records a live run and a replayed one.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;
use z_engine_provider::{ChatProvider, ChatRequest, EventStream, StreamEvent};

use super::entry::canonical_request;
use super::recorder::RunRecorder;

/// Wraps a provider and tapes everything that passes through it.
pub struct RecordingProvider {
    inner: Arc<dyn ChatProvider>,
    recorder: Arc<RunRecorder>,
}

impl std::fmt::Debug for RecordingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordingProvider")
            .field("recorder", &self.recorder)
            .finish_non_exhaustive()
    }
}

impl RecordingProvider {
    pub fn new(inner: Arc<dyn ChatProvider>, recorder: Arc<RunRecorder>) -> Self {
        Self { inner, recorder }
    }
}

impl ChatProvider for RecordingProvider {
    fn stream_chat(&self, request: &ChatRequest, abort: Arc<AtomicBool>) -> EventStream {
        let sequence = self.recorder.begin_exchange();
        let hash = match canonical_request(request) {
            Ok((_, hash)) => hash,
            Err(err) => {
                // A request that cannot be serialized cannot be matched
                // later either; tape it as unmatchable rather than
                // silently taping a lie.
                tracing::error!(%err, "cassette: request could not be hashed");
                String::new()
            }
        };
        let mut upstream = self.inner.stream_chat(request, abort);
        let (tx, rx) = mpsc::channel(64);
        let recorder = Arc::clone(&self.recorder);
        let request = request.clone();
        let handle = tokio::spawn(async move {
            let mut events: Vec<StreamEvent> = Vec::new();
            let mut error = None;
            while let Some(item) = upstream.recv().await {
                match &item {
                    Ok(event) => events.push(event.clone()),
                    Err(err) => error = Some(err.to_string()),
                }
                if tx.send(item).await.is_err() {
                    // The consumer gave up (abort, shutdown); tape what
                    // actually reached it.
                    break;
                }
            }
            recorder.finish_exchange(sequence, &request, &hash, &events, error);
        });
        // The exchange lands when the stream *ends*, which is after the
        // consumer stops reading at the finish event. Hand the task to
        // the recorder so the run can wait for its own tape.
        self.recorder.track(handle);
        rx
    }

    fn set_api_key(&self, key: Option<String>) {
        self.inner.set_api_key(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::tape::RunCassette;
    use std::sync::atomic::Ordering;
    use z_engine_provider::{ChatMessage, FinishReason, ProviderError, Usage};

    struct Canned {
        events: Vec<Result<StreamEvent, ProviderError>>,
        seen: std::sync::Mutex<Vec<ChatRequest>>,
    }

    impl ChatProvider for Canned {
        fn stream_chat(&self, request: &ChatRequest, _abort: Arc<AtomicBool>) -> EventStream {
            self.seen.lock().unwrap().push(request.clone());
            let (tx, rx) = mpsc::channel(16);
            for event in &self.events {
                let item = match event {
                    Ok(e) => Ok(e.clone()),
                    Err(e) => Err(ProviderError::StreamInterrupted(e.to_string())),
                };
                tx.try_send(item).unwrap();
            }
            rx
        }

        fn set_api_key(&self, _key: Option<String>) {}
    }

    fn canned(events: Vec<Result<StreamEvent, ProviderError>>) -> Arc<Canned> {
        Arc::new(Canned {
            events,
            seen: std::sync::Mutex::new(Vec::new()),
        })
    }

    async fn drain(mut rx: EventStream) -> Vec<Result<StreamEvent, ProviderError>> {
        let mut out = Vec::new();
        while let Some(item) = rx.recv().await {
            out.push(item);
        }
        out
    }

    #[tokio::test]
    async fn the_tap_forwards_every_event_and_records_the_exchange() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        let inner = canned(vec![
            Ok(StreamEvent::TextDelta("hi".into())),
            Ok(StreamEvent::Usage(Usage {
                prompt_tokens: 3,
                completion_tokens: 2,
            })),
            Ok(StreamEvent::Finish(FinishReason::Stop)),
            Ok(StreamEvent::Done),
        ]);
        let tap = RecordingProvider::new(Arc::clone(&inner) as Arc<dyn ChatProvider>, recorder);
        let request = ChatRequest::new("m", vec![ChatMessage::user("hi")]);

        let seen = drain(tap.stream_chat(&request, Arc::new(AtomicBool::new(false)))).await;
        assert_eq!(seen.len(), 4, "the tap must not swallow events");

        let cassette = RunCassette::load(&path).unwrap();
        let exchange = &cassette.exchanges()[0];
        assert_eq!(exchange.sequence, 0);
        assert_eq!(exchange.request, request);
        assert_eq!(exchange.events.len(), 4);
        assert!(exchange.error.is_none());
        assert_eq!(cassette.metrics(), None, "metrics land on turn end");
    }

    /// A provider whose stream ends a beat *after* the finish event,
    /// like a real client that polls once more before dropping its
    /// sender.
    struct Trailing;

    impl ChatProvider for Trailing {
        fn stream_chat(&self, _request: &ChatRequest, _abort: Arc<AtomicBool>) -> EventStream {
            let (tx, rx) = mpsc::channel(16);
            tokio::spawn(async move {
                let usage = Usage {
                    prompt_tokens: 7,
                    completion_tokens: 5,
                };
                let _ = tx.send(Ok(StreamEvent::Usage(usage))).await;
                let _ = tx.send(Ok(StreamEvent::Done)).await;
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            });
            rx
        }

        fn set_api_key(&self, _key: Option<String>) {}
    }

    #[tokio::test]
    async fn settling_waits_for_an_exchange_the_consumer_stopped_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        let tap = RecordingProvider::new(
            Arc::new(Trailing) as Arc<dyn ChatProvider>,
            Arc::clone(&recorder),
        );

        let mut stream = tap.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("hi")]),
            Arc::new(AtomicBool::new(false)),
        );
        // The agent's consumer stops at the finish event and leaves the
        // channel open, so the exchange is still in flight here.
        while let Some(item) = stream.recv().await {
            if matches!(item, Ok(StreamEvent::Done)) {
                break;
            }
        }
        assert!(
            RunCassette::load(&path).unwrap().exchanges().is_empty(),
            "the tap records at stream end, so nothing is taped yet"
        );

        recorder.settle().await;

        assert_eq!(
            RunCassette::load(&path).unwrap().exchanges().len(),
            1,
            "settling must not return before the tape is whole"
        );
        recorder.record_turn("completed");
        let metrics = RunCassette::load(&path).unwrap().metrics().unwrap();
        assert_eq!(
            (metrics.input_tokens, metrics.output_tokens),
            (7, 5),
            "metrics snapshotted before the last exchange land would undercount"
        );
    }

    #[tokio::test]
    async fn a_failed_stream_is_taped_as_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        let inner = canned(vec![
            Ok(StreamEvent::TextDelta("par".into())),
            Err(ProviderError::StreamInterrupted("upstream exploded".into())),
        ]);
        let tap = RecordingProvider::new(inner as Arc<dyn ChatProvider>, recorder);

        drain(tap.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("hi")]),
            Arc::new(AtomicBool::new(false)),
        ))
        .await;

        let exchange = RunCassette::load(&path).unwrap().exchanges().remove(0);
        assert_eq!(exchange.events.len(), 1);
        assert!(
            exchange.error.unwrap().contains("upstream exploded"),
            "a truncated success would replay as a lie"
        );
    }

    #[tokio::test]
    async fn the_abort_flag_and_key_reach_the_wrapped_provider() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = RunRecorder::recording(dir.path().join("run.jsonl")).unwrap();
        let inner = canned(vec![Ok(StreamEvent::Done)]);
        let tap = RecordingProvider::new(Arc::clone(&inner) as Arc<dyn ChatProvider>, recorder);
        let abort = Arc::new(AtomicBool::new(true));

        drain(tap.stream_chat(
            &ChatRequest::new("m", vec![ChatMessage::user("hi")]),
            Arc::clone(&abort),
        ))
        .await;

        assert!(abort.load(Ordering::SeqCst));
        assert_eq!(inner.seen.lock().unwrap().len(), 1);
        tap.set_api_key(None);
    }
}
