//! Content-addressed evidence storage: a filesystem CAS for artifact
//! bytes ([`FsBlobStore`]) plus an append-only, human-readable transcript
//! of [`EvidenceRecord`]s ([`EvidenceLedger`]) recorded per run at
//! `.z-engine/runs/<run-id>/`. This module only stores and replays
//! evidence; it has no knowledge of tools, agents, or the UI.

mod blob;
mod error;
mod ledger;
mod record;

pub use blob::{BlobHandle, BlobStore, FsBlobStore};
pub use error::EvidenceError;
pub use ledger::EvidenceLedger;
pub use record::EvidenceRecord;
