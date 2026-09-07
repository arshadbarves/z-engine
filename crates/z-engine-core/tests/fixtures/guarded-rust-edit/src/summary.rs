//! Out of scope on purpose: the work order in the slice test names only
//! `src/lib.rs`, so an edit here must be refused by the mutation gate.

/// Render a one-line summary of `text`.
pub fn summarize(text: &str) -> String {
    format!("{} words", crate::word_count(text))
}
