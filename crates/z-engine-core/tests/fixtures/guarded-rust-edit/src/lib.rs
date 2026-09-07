//! The one file a guarded run of this fixture may write.
//!
//! `word_count` counts bytes, which compiles and therefore passes
//! `cargo check`; only the held-out test in `tests/held_out.rs` says so.

mod summary;

pub use summary::summarize;

/// Count the words in `text`.
pub fn word_count(text: &str) -> usize {
    text.len()
}
