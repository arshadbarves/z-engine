//! Session persistence v2: per-session logs, subagent transcripts,
//! artifacts, metadata, and import of v1 transcripts.
//!
//! Under the sessions directory: `<id>/log.jsonl` (records),
//! `<id>/meta.json` (listing cache), `<id>/agents/<agent>.jsonl` (subagent
//! transcripts), `<id>/artifacts/`, and untouched v1 `<ULID>.jsonl` files.

mod append;
mod artifacts;
mod durable;
mod error;
mod heal;
mod layout;
mod legacy;
mod listing;
mod log;
mod meta;
mod read;
mod record;
mod replay;
mod store;

pub use artifacts::Artifacts;
pub use error::StoreError;
pub use legacy::import_v1;
pub use log::{AgentLog, SessionLog};
pub use meta::SessionMeta;
pub use read::{ReadOutcome, read_records};
pub use record::{LogRecord, SESSION_SCHEMA};
pub use replay::{ReplayState, replay};
pub use store::{LoadedSession, NewSession, SessionStore};
