//! Held out from the run: the agent never reads or writes this file, and
//! the work order may not touch it. It is the only thing that can tell a
//! byte count from a word count, so the change is proven by verification
//! rather than by the model saying it is done.

use guarded_rust_edit::{summarize, word_count};

#[test]
fn word_count_counts_words_not_bytes() {
    assert_eq!(word_count("one two three"), 3);
}

#[test]
fn the_summary_reports_the_word_count() {
    assert_eq!(summarize("one two three"), "3 words");
}
