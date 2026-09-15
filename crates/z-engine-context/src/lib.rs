//! Provider-independent context projection; task authority remains with the harness.

mod builder;
mod error;
mod notes;
mod packet;
mod report;

pub use builder::build_packet;
pub use error::ContextError;
pub use notes::{ModelNote, NoteKind, NoteSource, NoteTrust};
pub use packet::{
    BudgetUsage, CONTEXT_PACKET_SCHEMA_VERSION, ContextPacket, EvidenceDetails, EvidenceRef,
    Freshness, HarnessObservation, ObservationSource, OmittedCounts, PacketKind, PacketProvenance,
};
pub use report::{
    CheckObservation, CheckOutcome, EvidenceArtifact, Requirement, TaskObservation, TaskStatus,
};
