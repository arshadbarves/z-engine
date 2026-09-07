//! The run recorder: one object that a guarded run reports to, and the
//! only place a cassette is written.
//!
//! The agent loop, the tools, and the gates each hand it a fact as it
//! happens; the recorder allocates the sequence numbers, keeps the
//! running tally, and appends. In replay mode it also *serves* the one
//! arbitrary thing a guarded run mints — evidence ids — so a replayed
//! run produces the same request bytes as the run it is reproducing.
//!
//! Every append can fail, and a failed append means the tape no longer
//! describes the run. Rather than teach every caller to handle that,
//! the first failure is kept here ([`RunRecorder::fault`]), persisted
//! beside the cassette, and read once by the loop — which then refuses
//! to call the turn complete.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tokio::task::JoinHandle;

use z_engine_provider::{ChatMessage, ChatRequest, RequestLane, StreamEvent};

use super::counters::Sequences;
use super::entry::{
    CassetteEntry, CompletionRecord, EvidenceMint, GateDecision, GateKind, PromptHash,
    ProviderExchange, ToolDisposition, ToolOutcome, content_hash, prompt_hash,
};
use super::error::ReplayError;
use super::fault::{FaultLog, RecordingFault};
use super::ids::IdSource;
use super::settle::InFlight;
use super::sink::{CassetteWriter, EntrySink};
use super::tally::SharedTally;
use super::tape::RunCassette;

/// Records one run onto one cassette.
pub struct RunRecorder {
    sink: Box<dyn EntrySink>,
    ids: IdSource,
    sequences: Sequences,
    tally: SharedTally,
    /// The first entry this run failed to record, if any.
    faults: FaultLog,
    /// Forwarding tasks that still owe the tape an exchange.
    inflight: InFlight,
    started: Instant,
}

impl std::fmt::Debug for RunRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunRecorder")
            .field("replaying", &self.ids.is_replaying())
            .field("fault", &self.fault())
            .finish()
    }
}

impl RunRecorder {
    /// Record a live run onto a new cassette at `path`.
    pub fn recording(path: impl AsRef<Path>) -> Result<Arc<Self>, ReplayError> {
        let path = path.as_ref();
        Ok(Arc::new(Self::new(
            path,
            Box::new(CassetteWriter::create(path)?),
            IdSource::Fresh,
        )))
    }

    /// Record a replayed run onto a new cassette at `path`, taking the
    /// arbitrary parts of the run (evidence ids) from `source`.
    pub fn replaying(
        path: impl AsRef<Path>,
        source: &RunCassette,
    ) -> Result<Arc<Self>, ReplayError> {
        let path = path.as_ref();
        Ok(Arc::new(Self::new(
            path,
            Box::new(CassetteWriter::create(path)?),
            IdSource::from_cassette(source),
        )))
    }

    /// Record onto an arbitrary sink, marking `path` if it fails.
    ///
    /// The seam exists so a recorder can be *shown* a write failure:
    /// fail-closed behaviour that is never exercised is a claim rather
    /// than a property.
    pub fn onto(path: impl AsRef<Path>, sink: Box<dyn EntrySink>) -> Arc<Self> {
        Arc::new(Self::new(path.as_ref(), sink, IdSource::Fresh))
    }

    fn new(path: &Path, sink: Box<dyn EntrySink>, ids: IdSource) -> Self {
        Self {
            sink,
            ids,
            sequences: Sequences::default(),
            tally: SharedTally::default(),
            faults: FaultLog::beside(path),
            inflight: InFlight::default(),
            started: Instant::now(),
        }
    }

    /// True when this run's arbitrary values come from a recorded run.
    pub fn is_replaying(&self) -> bool {
        self.ids.is_replaying()
    }

    /// The first entry this run failed to record, if any. `Some` means
    /// the cassette is not the run, and the run may not claim otherwise.
    pub fn fault(&self) -> Option<RecordingFault> {
        self.faults.get()
    }

    /// Claim the next request position *on `lane`*. Called when a
    /// request is sent, so the cassette preserves issue order even if
    /// streams finish out of order.
    pub fn begin_exchange(&self, lane: &RequestLane) -> u64 {
        self.sequences.exchange(lane)
    }

    /// Append a finished exchange and fold its usage into the tally.
    pub fn finish_exchange(
        &self,
        lane: &RequestLane,
        sequence: u64,
        request: &ChatRequest,
        request_hash: &str,
        events: &[StreamEvent],
        error: Option<String>,
    ) {
        for event in events {
            if let StreamEvent::Usage(usage) = event {
                self.tally
                    .add_usage(&request.model, usage.prompt_tokens, usage.completion_tokens);
            }
        }
        self.append(CassetteEntry::Exchange(ProviderExchange {
            lane: lane.clone(),
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
            Err(err) => return self.note_fault("prompt", err.to_string()),
        };
        let sequence = self.sequences.prompt();
        self.append(CassetteEntry::Prompt(PromptHash { sequence, hash }));
    }

    /// Claim the next tool position, in the order the round's calls were
    /// *decided*. Concurrency-safe tools finish in whatever order the
    /// scheduler picks, and that order is not the run's.
    pub fn begin_tool(&self) -> u64 {
        self.sequences.tool()
    }

    /// Record what became of one tool call: the result it returned, or
    /// the refusal that stood in for it. A refused call shapes the next
    /// request exactly as much as one that ran.
    pub fn record_tool(
        &self,
        sequence: u64,
        name: &str,
        disposition: ToolDisposition,
        ok: bool,
        result: &str,
    ) {
        self.tally.add_tool_call();
        self.append(CassetteEntry::Tool(ToolOutcome {
            sequence,
            name: name.to_string(),
            disposition,
            ok,
            result_hash: content_hash(result.as_bytes()),
        }));
    }

    /// Record one gate ruling.
    pub fn record_gate(&self, kind: GateKind, target: &str, allowed: bool, reason: Option<String>) {
        let sequence = self.sequences.gate();
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
        let sequence = self.sequences.evidence();
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
        self.inflight.track(handle);
    }

    /// Wait until every exchange in flight has reached the tape. An
    /// exchange that never arrives is a lost entry, not a slow one.
    pub async fn settle(&self) {
        if let Some(fault) = self.inflight.settle().await {
            self.fail(fault);
        }
    }

    /// Record what the completion gate proved.
    pub fn record_completion(&self, manifest_hash: &str, diff_hash: Option<String>, verdict: &str) {
        let sequence = self.sequences.completion();
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
        let snapshot = self
            .tally
            .close_turn(outcome, self.started.elapsed().as_millis() as u64);
        self.append(CassetteEntry::Metrics(snapshot));
    }

    /// Report a fact that could not be recorded, from somewhere other
    /// than an append — a request that would not serialize, a stream
    /// that never reported back.
    pub fn note_fault(&self, entry: &str, detail: String) {
        self.fail(RecordingFault {
            kind: entry.to_string(),
            detail,
        });
    }

    /// Append one entry, or remember that the tape is no longer whole.
    fn append(&self, entry: CassetteEntry) {
        if let Err(err) = self.sink.append(&entry) {
            self.fail(RecordingFault {
                kind: entry.kind().to_string(),
                detail: err.to_string(),
            });
        }
    }

    /// Condemn the recording, and put a copy of the reason on the tape
    /// for a reader holding only the file.
    fn fail(&self, fault: RecordingFault) {
        if let Some(fault) = self.faults.condemn(fault)
            && let Err(err) = self.sink.append(&CassetteEntry::Fault(fault))
        {
            tracing::debug!(%err, "cassette: the fault could not be taped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::fault::marker_path;
    use crate::replay::sink::BrokenSink;
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
            let seq = recorder.begin_exchange(&RequestLane::MAIN);
            recorder.finish_exchange(
                &RequestLane::MAIN,
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
        let seq = recorder.begin_tool();
        recorder.record_tool(seq, "read_file", ToolDisposition::Executed, true, "body");
        recorder.record_turn("completed");

        let cassette = RunCassette::load(&path).unwrap();
        let metrics = cassette.metrics().unwrap();
        assert_eq!(metrics.model_id, "test-model");
        assert_eq!(metrics.input_tokens, 30);
        assert_eq!(metrics.output_tokens, 10);
        assert_eq!(metrics.turns, 1);
        assert_eq!(metrics.tool_calls, 1);
        assert_eq!(metrics.outcome, "completed");
        assert!(recorder.fault().is_none());
    }

    /// Refusals are part of the trajectory, so they are counted with
    /// everything else: the tally and the tape must not disagree about
    /// how many calls the model made.
    #[test]
    fn the_tool_counter_matches_the_calls_on_the_tape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();

        for (name, disposition, ok) in [
            ("read_file", ToolDisposition::Executed, true),
            ("bash", ToolDisposition::PlanRefused, false),
            ("write_file", ToolDisposition::UserDenied, false),
            ("edit_file", ToolDisposition::Abandoned, false),
        ] {
            let seq = recorder.begin_tool();
            recorder.record_tool(seq, name, disposition, ok, "text");
        }
        recorder.record_turn("blocked:completion");

        let cassette = RunCassette::load(&path).unwrap();
        let outcomes = cassette.tool_outcomes();
        assert_eq!(
            cassette.metrics().unwrap().tool_calls as usize,
            outcomes.len()
        );
        assert_eq!(outcomes.len(), 4);
        assert_eq!(
            outcomes.iter().filter(|o| o.disposition.ran()).count(),
            1,
            "only one of these reached a tool"
        );
        assert_eq!(
            outcomes.iter().map(|o| o.sequence).collect::<Vec<_>>(),
            [0, 1, 2, 3],
            "sequences follow decision order"
        );
    }

    /// A write that fails must leave a run that cannot pass for
    /// recorded: the fault is remembered, persisted beside the tape, and
    /// the tape refuses to load.
    #[test]
    fn a_failed_append_condemns_the_recording_instead_of_being_logged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::onto(&path, Box::new(BrokenSink));

        let seq = recorder.begin_tool();
        recorder.record_tool(seq, "write_file", ToolDisposition::Executed, true, "done");

        let fault = recorder.fault().expect("a lost entry is a fault");
        assert_eq!(fault.kind, "tool");
        assert!(fault.detail.contains("no space left"), "{fault}");
        assert!(marker_path(&path).exists(), "the marker must be persisted");
    }

    /// Only the first loss is kept: later ones are downstream of it and
    /// would bury the cause.
    #[test]
    fn the_first_lost_entry_is_the_one_reported() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = RunRecorder::onto(dir.path().join("run.jsonl"), Box::new(BrokenSink));
        recorder.record_prompt(&[ChatMessage::system("L0")]);
        recorder.record_turn("completed");
        assert_eq!(recorder.fault().unwrap().kind, "prompt");
    }
}
