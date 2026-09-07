//! Record and replay complete guarded runs.
//!
//! A cassette is the whole run as facts: the exact requests and the
//! events they produced, the prompt hashes that framed them, what each
//! tool returned, how the mutation gate ruled, which evidence ids were
//! minted, what the completion gate proved, and what the run cost.
//! Recording is a tap ([`RecordingProvider`]) around any
//! [`ChatProvider`](z_engine_provider::ChatProvider), so it records a
//! live run and a replayed one the same way; replay
//! ([`ReplayProvider`]) is the cassette *as* the transport, with no
//! network underneath it at all.
//!
//! Replay matches on the exact serialized request, in sequence. The
//! first request that does not match ends the run with a typed failure
//! carrying its sequence: there is no fallback that would let a
//! divergent run finish looking like the recorded one.
//!
//! Recording never weakens a gate. A replayed run re-runs its own
//! verification, re-asks its own mutation gate, and re-derives its own
//! diff; the cassette supplies only what the provider said and the ids
//! the run had no way to derive.

mod entry;
mod error;
mod fingerprint;
mod ids;
mod recorder;
mod recording;
mod replaying;
mod tape;

pub use entry::{
    CassetteEntry, CompletionRecord, EvidenceMint, GateDecision, GateKind, MetricsFingerprint,
    PromptHash, ProviderExchange, RunMetrics, ToolOutcome, canonical_request, content_hash,
    prompt_hash,
};
pub use error::ReplayError;
pub use fingerprint::{diff_fingerprint, manifest_fingerprint};
pub use recorder::RunRecorder;
pub use recording::RecordingProvider;
pub use replaying::{ReplayMismatch, ReplayProvider};
pub use tape::RunCassette;
