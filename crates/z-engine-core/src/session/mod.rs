//! Append-only JSONL transcripts with durable task evidence and crash-aware replay.

mod catalog;
mod reader;
mod replay;
mod storage;
mod title;
mod trim;
mod types;
mod unread;
mod writer;

pub use catalog::{delete_session, list_sessions};
pub use reader::read_events;
pub use replay::replay;
pub use title::{display_title, fallback_title};
pub use trim::{events_before_user_turn, trim_file_before_user_turn};
pub use types::{PersistedToolCall, Replayed, SessionEvent, SessionSummary};
pub use unread::unread_outcome;
pub use writer::SessionWriter;

#[cfg(test)]
mod tests;
