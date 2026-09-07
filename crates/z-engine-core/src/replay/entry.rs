//! What a cassette holds: one typed entry per fact a run produced, and
//! the hashes those entries are compared on.
//!
//! Entries are deliberately *facts about a run*, never wall-clock or
//! machine detail: a request and the events it produced, the hash of the
//! prompt prefix that framed it, what each tool returned, how the
//! mutation gate ruled, which evidence ids were minted, what the
//! completion gate proved, and the run's metrics. Two runs of the same
//! work must produce byte-identical entries; the only field that may
//! differ is [`RunMetrics::wall_time_ms`], which is why the comparable
//! projection ([`RunMetrics::deterministic`]) leaves it out.
//!
//! Hashing goes through the evidence module's [`BlobHandle`], so "same
//! content" means one thing across evidence, mutations, and tapes.

use serde::{Deserialize, Serialize};

use z_engine_provider::{ChatMessage, ChatRequest, RequestLane, StreamEvent};

use crate::evidence::BlobHandle;

use super::error::ReplayError;
use super::fault::RecordingFault;

/// SHA-256 hex of `bytes`, reusing the evidence module's content hash.
pub fn content_hash(bytes: &[u8]) -> String {
    BlobHandle::of(bytes).to_string()
}

/// The exact bytes a request serializes to, and their hash. Matching is
/// done on these bytes: a replay that accepted anything less would be
/// asserting the request was *close enough*.
pub fn canonical_request(request: &ChatRequest) -> Result<(String, String), ReplayError> {
    let text =
        serde_json::to_string(request).map_err(|source| ReplayError::Serialize { source })?;
    let hash = content_hash(text.as_bytes());
    Ok((text, hash))
}

/// Hash of the prompt prefix framing one request (L0, repo map, notes,
/// work-order digest) — the part that is the harness's own doing rather
/// than the conversation's.
pub fn prompt_hash(prefix: &[ChatMessage]) -> Result<String, ReplayError> {
    let text = serde_json::to_string(prefix).map_err(|source| ReplayError::Serialize { source })?;
    Ok(content_hash(text.as_bytes()))
}

/// One request and everything the provider streamed back for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExchange {
    /// Which logical stream this request belongs to. Requests are
    /// ordered *within* a lane; the turn loop, a session title and a
    /// sub-agent share one provider handle and have no order relative to
    /// each other, so a single global sequence would record the
    /// scheduler rather than the run.
    #[serde(default)]
    pub lane: RequestLane,
    /// Position in this lane's request sequence, allocated when the
    /// request was issued (not when the stream finished).
    pub sequence: u64,
    /// Hash of the exact serialized request, as it was sent.
    pub request_hash: String,
    pub request: ChatRequest,
    pub events: Vec<StreamEvent>,
    /// Set when the recorded stream ended in a provider failure, so a
    /// replay reproduces the failure instead of a truncated success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The prompt prefix that framed one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptHash {
    pub sequence: u64,
    pub hash: String,
}

/// What became of one tool call the model asked for.
///
/// A call the harness refused shapes the next request exactly as much as
/// one it ran — the model is handed a refusal instead of a result and
/// carries on from there — so refusals are facts about the run, not
/// absences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolDisposition {
    /// Dispatched to the registry. `ok` then says whether it worked;
    /// an unknown tool name is an executed call that failed.
    Executed,
    /// The user answered the approval prompt with "no".
    UserDenied,
    /// Plan mode refuses mutating tools before they are ever offered.
    PlanRefused,
    /// The round was abandoned (abort or shutdown) before this call ran.
    Abandoned,
    /// The model's arguments never parsed, so nothing was dispatched.
    Malformed,
}

impl ToolDisposition {
    /// True when the call reached a tool.
    pub fn ran(&self) -> bool {
        matches!(self, ToolDisposition::Executed)
    }
}

/// What one tool call returned, by content rather than by transcript
/// text, so a cassette stays small and comparisons stay exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOutcome {
    /// Position in the round's call order, claimed when the call was
    /// *decided* — concurrency-safe tools finish in whatever order the
    /// scheduler picks, and that order is not the run's.
    pub sequence: u64,
    pub name: String,
    /// What the harness did with the call. Older tapes predate the
    /// field and only ever held executed calls.
    #[serde(default = "executed")]
    pub disposition: ToolDisposition,
    pub ok: bool,
    /// Hash of the text handed back to the model — the tool's result,
    /// or the refusal that stood in for it.
    pub result_hash: String,
}

fn executed() -> ToolDisposition {
    ToolDisposition::Executed
}

/// Which gate ruled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GateKind {
    /// A write about to reach disk.
    Mutation,
    /// A shell command about to run.
    Command,
}

/// One ruling of the guarded mutation gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateDecision {
    pub sequence: u64,
    pub kind: GateKind,
    /// Repository-relative path, or the command line.
    pub target: String,
    pub allowed: bool,
    /// The refusal, verbatim, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// An evidence id this run minted, and the read it was minted for.
///
/// Replay serves these back: an id is the one piece of a guarded run
/// that is genuinely arbitrary, and it reaches the model inside every
/// `read_file` result, so a replay that minted fresh ones would produce
/// a different second request and diverge from itself.
///
/// The path *and* the range identify the read, because one round can
/// read two windows of the same file concurrently and the order those
/// reads finish in is the scheduler's business, not the run's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceMint {
    pub sequence: u64,
    /// Repository-relative path the evidence was captured from.
    pub path: String,
    /// Inclusive 1-based line range captured; `None` for a whole file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<(u32, u32)>,
    pub id: String,
}

/// What the completion gate decided, and what the workspace held when it
/// did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionRecord {
    pub sequence: u64,
    /// Hash of the manifest's verdict-bearing content — scope, change
    /// set, breaches, and each check's status. Durations and captured
    /// output are excluded: they are the same verdict with different
    /// weather.
    pub manifest_hash: String,
    /// Hash of the run's change set (paths plus content hashes), or
    /// `None` when it changed nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_hash: Option<String>,
    pub verdict: String,
}

/// What the run cost and how it ended: see [`super::tally`].
pub use super::tally::{MetricsFingerprint, RunMetrics};

/// One line of a cassette.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "camelCase")]
pub enum CassetteEntry {
    Exchange(ProviderExchange),
    Prompt(PromptHash),
    Tool(ToolOutcome),
    Gate(GateDecision),
    Evidence(EvidenceMint),
    Completion(CompletionRecord),
    Metrics(RunMetrics),
    /// The recorder could not write something down. Appended
    /// best-effort — the same failure that lost the entry usually loses
    /// this too, which is why it is also persisted beside the tape (see
    /// [`super::fault`]).
    Fault(RecordingFault),
}

impl CassetteEntry {
    /// Which kind of fact this is, for reporting what went unrecorded.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Exchange(_) => "exchange",
            Self::Prompt(_) => "prompt",
            Self::Tool(_) => "tool",
            Self::Gate(_) => "gate",
            Self::Evidence(_) => "evidence",
            Self::Completion(_) => "completion",
            Self::Metrics(_) => "metrics",
            Self::Fault(_) => "fault",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_hash_is_over_the_exact_serialized_request() {
        let request = ChatRequest::new("m", vec![ChatMessage::user("hi")]);
        let (text, hash) = canonical_request(&request).unwrap();
        assert_eq!(hash, content_hash(text.as_bytes()));
        let other = ChatRequest::new("m", vec![ChatMessage::user("hi ")]);
        assert_ne!(canonical_request(&other).unwrap().1, hash);
    }

    #[test]
    fn the_prompt_hash_follows_the_prefix_it_was_taken_from() {
        let prefix = vec![ChatMessage::system("L0"), ChatMessage::system("map")];
        assert_eq!(prompt_hash(&prefix).unwrap(), prompt_hash(&prefix).unwrap());
        assert_ne!(
            prompt_hash(&prefix).unwrap(),
            prompt_hash(&prefix[..1]).unwrap()
        );
    }

    /// Tapes written before lanes and dispositions existed described a
    /// single-lane run of executed calls; they must still read as that
    /// rather than failing the load.
    #[test]
    fn an_older_entry_reads_as_the_main_lane_and_an_executed_call() {
        let exchange: ProviderExchange = serde_json::from_str(
            r#"{"sequence":0,"requestHash":"h","request":{"model":"m","messages":[],"stream":true,"stream_options":{"include_usage":true}},"events":[]}"#,
        )
        .unwrap();
        assert_eq!(exchange.lane, RequestLane::MAIN);

        let tool: ToolOutcome =
            serde_json::from_str(r#"{"sequence":0,"name":"read_file","ok":true,"resultHash":"h"}"#)
                .unwrap();
        assert_eq!(tool.disposition, ToolDisposition::Executed);
        assert!(tool.disposition.ran());
    }

    #[test]
    fn a_refused_call_is_a_recorded_fact_of_its_own_kind() {
        assert!(!ToolDisposition::UserDenied.ran());
        assert!(!ToolDisposition::PlanRefused.ran());
        assert!(!ToolDisposition::Abandoned.ran());
        assert!(!ToolDisposition::Malformed.ran());
        let entry = CassetteEntry::Tool(ToolOutcome {
            sequence: 0,
            name: "bash".into(),
            disposition: ToolDisposition::PlanRefused,
            ok: false,
            result_hash: content_hash(b"refused"),
        });
        assert_eq!(entry.kind(), "tool");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"planRefused\""), "{json}");
    }

    #[test]
    fn every_entry_can_name_its_own_kind() {
        assert_eq!(
            CassetteEntry::Fault(RecordingFault {
                kind: "exchange".into(),
                detail: "disk full".into(),
            })
            .kind(),
            "fault"
        );
        assert_eq!(
            CassetteEntry::Prompt(PromptHash {
                sequence: 0,
                hash: content_hash(b"p"),
            })
            .kind(),
            "prompt"
        );
    }
}
