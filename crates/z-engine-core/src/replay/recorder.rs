//! The run recorder: one object that a guarded run reports to, and the
//! only place a cassette is written.
//!
//! The agent loop, the tools, and the gates each hand it a fact as it
//! happens; the recorder allocates the sequence numbers, keeps the
//! running metrics, and appends. In replay mode it also *serves* the one
//! arbitrary thing a guarded run mints — evidence ids — so a replayed
//! run produces the same request bytes as the run it is reproducing.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;

use z_engine_provider::{ChatMessage, ChatRequest, StreamEvent};

use super::entry::{
    CassetteEntry, CompletionRecord, EvidenceMint, GateDecision, GateKind, PromptHash,
    ProviderExchange, RunMetrics, ToolOutcome, content_hash, prompt_hash,
};
use super::error::ReplayError;
use super::ids::IdSource;
use super::tape::{CassetteWriter, RunCassette};

/// Where the run's evidence ids come from.
/// Per-kind positions. Kept separate so "the third request" and "the
/// third tool call" each mean what they say, and so a mismatch reports
/// the request number rather than a line number.
#[derive(Default)]
struct Counters {
    exchanges: u64,
    prompts: u64,
    tools: u64,
    gates: u64,
    evidence: u64,
    completions: u64,
}

#[derive(Default)]
struct MetricsState {
    model_id: String,
    input_tokens: u64,
    output_tokens: u64,
    turns: u64,
    tool_calls: u64,
    outcome: String,
}

/// Records one run onto one cassette.
pub struct RunRecorder {
    writer: CassetteWriter,
    ids: IdSource,
    counters: Mutex<Counters>,
    metrics: Mutex<MetricsState>,
    /// Forwarding tasks that still owe the tape an exchange.
    inflight: Mutex<Vec<JoinHandle<()>>>,
    started: Instant,
}

/// How long [`RunRecorder::settle`] waits for one stream to finish
/// reporting before recording that it did not. Bounded so a wedged
/// provider costs the run a known gap in its tape rather than the turn.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);

impl std::fmt::Debug for RunRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunRecorder")
            .field("replaying", &self.ids.is_replaying())
            .finish()
    }
}

impl RunRecorder {
    /// Record a live run onto a new cassette at `path`.
    pub fn recording(path: impl AsRef<Path>) -> Result<Arc<Self>, ReplayError> {
        Ok(Arc::new(Self::open(path.as_ref(), IdSource::Fresh)?))
    }

    /// Record a replayed run onto a new cassette at `path`, taking the
    /// arbitrary parts of the run (evidence ids) from `source`.
    pub fn replaying(
        path: impl AsRef<Path>,
        source: &RunCassette,
    ) -> Result<Arc<Self>, ReplayError> {
        let ids = IdSource::from_cassette(source);
        Ok(Arc::new(Self::open(path.as_ref(), ids)?))
    }

    fn open(path: &Path, ids: IdSource) -> Result<Self, ReplayError> {
        Ok(Self {
            writer: CassetteWriter::create(path)?,
            ids,
            counters: Mutex::new(Counters::default()),
            metrics: Mutex::new(MetricsState::default()),
            inflight: Mutex::new(Vec::new()),
            started: Instant::now(),
        })
    }

    /// True when this run's arbitrary values come from a recorded run.
    pub fn is_replaying(&self) -> bool {
        self.ids.is_replaying()
    }

    /// Claim the next request position. Called when a request is *sent*,
    /// so the cassette preserves issue order even if streams finish out
    /// of order.
    pub fn begin_exchange(&self) -> u64 {
        let mut counters = self.counters.lock().expect("recorder counters");
        let seq = counters.exchanges;
        counters.exchanges += 1;
        seq
    }

    /// Append a finished exchange and fold its usage into the metrics.
    pub fn finish_exchange(
        &self,
        sequence: u64,
        request: &ChatRequest,
        request_hash: &str,
        events: &[StreamEvent],
        error: Option<String>,
    ) {
        {
            let mut metrics = self.metrics.lock().expect("recorder metrics");
            if metrics.model_id.is_empty() {
                metrics.model_id = request.model.clone();
            }
            for event in events {
                if let StreamEvent::Usage(usage) = event {
                    metrics.input_tokens += usage.prompt_tokens;
                    metrics.output_tokens += usage.completion_tokens;
                }
            }
        }
        self.append(CassetteEntry::Exchange(ProviderExchange {
            sequence,
            request_hash: request_hash.to_string(),
            request: request.clone(),
            events: events.to_vec(),
            error,
        }));
    }

    /// Record the hash of the prompt prefix framing one request.
    pub fn record_prompt(&self, prefix: &[ChatMessage]) {
        let hash = match prompt_hash(prefix) {
            Ok(hash) => hash,
            Err(err) => {
                tracing::error!(%err, "cassette: prompt prefix could not be hashed");
                return;
            }
        };
        let sequence = self.next(|c| &mut c.prompts);
        self.append(CassetteEntry::Prompt(PromptHash { sequence, hash }));
    }

    /// Record what a tool returned.
    pub fn record_tool(&self, name: &str, ok: bool, result: &str) {
        self.metrics.lock().expect("recorder metrics").tool_calls += 1;
        let sequence = self.next(|c| &mut c.tools);
        self.append(CassetteEntry::Tool(ToolOutcome {
            sequence,
            name: name.to_string(),
            ok,
            result_hash: content_hash(result.as_bytes()),
        }));
    }

    /// Record one gate ruling.
    pub fn record_gate(&self, kind: GateKind, target: &str, allowed: bool, reason: Option<String>) {
        let sequence = self.next(|c| &mut c.gates);
        self.append(CassetteEntry::Gate(GateDecision {
            sequence,
            kind,
            target: target.to_string(),
            allowed,
            reason,
        }));
    }

    /// The evidence id for a read of `path` over `range`: fresh while
    /// recording, the recorded one while replaying.
    ///
    /// Either way the id goes on this run's own tape, so a replay is
    /// itself replayable.
    pub fn evidence_id(
        &self,
        path: &str,
        range: Option<(u32, u32)>,
    ) -> Result<String, ReplayError> {
        let id = self.ids.claim(path, range)?;
        let sequence = self.next(|c| &mut c.evidence);
        self.append(CassetteEntry::Evidence(EvidenceMint {
            sequence,
            path: path.to_string(),
            range,
            id: id.clone(),
        }));
        Ok(id)
    }

    /// Register a forwarding task that owes the tape an exchange.
    pub fn track(&self, handle: JoinHandle<()>) {
        self.inflight
            .lock()
            .expect("recorder inflight")
            .push(handle);
    }

    /// Wait until every exchange in flight has reached the tape.
    ///
    /// An exchange is appended when its stream *ends*, which can be
    /// after the turn that issued it has moved on — the consumer stops
    /// reading at the finish event, not at channel close. Without this
    /// barrier a run's own last request can be missing from its record,
    /// and the metrics can be snapshotted before its tokens are counted:
    /// a tape that is quietly short is worse than no tape at all.
    pub async fn settle(&self) {
        let handles: Vec<JoinHandle<()>> =
            std::mem::take(&mut *self.inflight.lock().expect("recorder inflight"));
        for handle in handles {
            match tokio::time::timeout(SETTLE_TIMEOUT, handle).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => tracing::error!(%err, "cassette: a recording task failed"),
                Err(_) => tracing::error!(
                    "cassette: a recording task did not finish in time; its exchange is missing"
                ),
            }
        }
    }

    /// Record what the completion gate proved.
    pub fn record_completion(&self, manifest_hash: &str, diff_hash: Option<String>, verdict: &str) {
        let sequence = self.next(|c| &mut c.completions);
        self.append(CassetteEntry::Completion(CompletionRecord {
            sequence,
            manifest_hash: manifest_hash.to_string(),
            diff_hash,
            verdict: verdict.to_string(),
        }));
    }

    /// Close out a turn: metrics are appended per turn, so a run that is
    /// killed mid-flight still leaves the tape it earned.
    pub fn record_turn(&self, outcome: &str) {
        let snapshot = {
            let mut metrics = self.metrics.lock().expect("recorder metrics");
            metrics.turns += 1;
            metrics.outcome = outcome.to_string();
            RunMetrics {
                model_id: metrics.model_id.clone(),
                input_tokens: metrics.input_tokens,
                output_tokens: metrics.output_tokens,
                turns: metrics.turns,
                tool_calls: metrics.tool_calls,
                wall_time_ms: self.started.elapsed().as_millis() as u64,
                outcome: metrics.outcome.clone(),
            }
        };
        self.append(CassetteEntry::Metrics(snapshot));
    }

    fn next(&self, field: impl Fn(&mut Counters) -> &mut u64) -> u64 {
        let mut counters = self.counters.lock().expect("recorder counters");
        let slot = field(&mut counters);
        let seq = *slot;
        *slot += 1;
        seq
    }

    /// A failed append leaves an incomplete tape, which replay will later
    /// refuse — loudly, and in the safe direction — so recording never
    /// takes the run down with it.
    fn append(&self, entry: CassetteEntry) {
        if let Err(err) = self.writer.append(&entry) {
            tracing::error!(%err, "cassette: entry could not be appended");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z_engine_provider::Usage;

    fn request(model: &str) -> ChatRequest {
        ChatRequest::new(model, vec![ChatMessage::user("hi")])
    }

    #[test]
    fn metrics_accumulate_usage_across_exchanges_and_turns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();

        for tokens in [(10, 5), (20, 5)] {
            let seq = recorder.begin_exchange();
            recorder.finish_exchange(
                seq,
                &request("test-model"),
                "hash",
                &[StreamEvent::Usage(Usage {
                    prompt_tokens: tokens.0,
                    completion_tokens: tokens.1,
                })],
                None,
            );
        }
        recorder.record_tool("read_file", true, "body");
        recorder.record_turn("completed");

        let metrics = RunCassette::load(&path).unwrap().metrics().unwrap();
        assert_eq!(metrics.model_id, "test-model");
        assert_eq!(metrics.input_tokens, 30);
        assert_eq!(metrics.output_tokens, 10);
        assert_eq!(metrics.turns, 1);
        assert_eq!(metrics.tool_calls, 1);
        assert_eq!(metrics.outcome, "completed");
    }

    #[test]
    fn exchange_sequences_are_claimed_in_issue_order() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = RunRecorder::recording(dir.path().join("run.jsonl")).unwrap();
        assert_eq!(recorder.begin_exchange(), 0);
        assert_eq!(recorder.begin_exchange(), 1);
        assert_eq!(recorder.begin_exchange(), 2);
    }
}
