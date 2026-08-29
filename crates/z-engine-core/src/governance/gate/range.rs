//! Localizing a change: which lines of the *old* file does turning it
//! into the new one touch? Pure text arithmetic, no diff library and no
//! I/O, so mutating tools can ask before they write.

use super::facts::LineRange;

/// The 1-based inclusive span of `old` that turning it into `new`
/// touches, or `None` when `old` is empty (a created file — all of it is
/// new, and nothing about it can have been read).
///
/// The common prefix and suffix are untouched, so everything between them
/// is the change. Pure insertions collapse to the single line they are
/// inserted at, and the span is clamped to `old`'s extent — so an append,
/// or a write that changes nothing at all, names `old`'s last line rather
/// than a line that does not exist yet.
pub fn changed_line_range(old: &str, new: &str) -> Option<LineRange> {
    // `split_inclusive` keeps each line's terminator, so a change that only
    // rewrites line endings (LF → CRLF) still counts as a change. `lines()`
    // strips `\r`, which would hide those bytes from the comparison and let
    // an unevidenced line be rewritten.
    let o: Vec<&str> = old.split_inclusive('\n').collect();
    let n: Vec<&str> = new.split_inclusive('\n').collect();
    if o.is_empty() {
        return None;
    }
    let mut prefix = 0;
    while prefix < o.len() && prefix < n.len() && o[prefix] == n[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < o.len() - prefix
        && suffix < n.len().saturating_sub(prefix)
        && o[o.len() - 1 - suffix] == n[n.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let first = (prefix + 1).min(o.len()) as u32;
    let last = (o.len() - suffix).max(prefix + 1).min(o.len()) as u32;
    Some((first, last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localizes_replacements_insertions_and_deletions() {
        assert_eq!(
            changed_line_range("a\nb\nc\n", "a\nB\nc\n"),
            Some((2, 2)),
            "replacement"
        );
        assert_eq!(
            changed_line_range("a\nb\n", "a\nx\nb\n"),
            Some((2, 2)),
            "insertion"
        );
        assert_eq!(
            changed_line_range("a\nb\nc\n", "a\nc\n"),
            Some((2, 2)),
            "deletion"
        );
        assert_eq!(
            changed_line_range("a\nb\nc\n", "A\nB\nC\n"),
            Some((1, 3)),
            "whole file"
        );
        assert_eq!(
            changed_line_range("a\nb\nc\n", ""),
            Some((1, 3)),
            "truncation to nothing"
        );
    }

    #[test]
    fn appends_stay_inside_the_old_file_and_creations_report_no_span() {
        assert_eq!(changed_line_range("a\n", "a\nb\n"), Some((1, 1)), "append");
        assert_eq!(changed_line_range("", "new\n"), None, "created file");
        // Degenerate cases clamp to the end of the old file, the same way
        // an append does: they never name a line that does not exist.
        assert_eq!(
            changed_line_range("a\nb\n", "a\nb\n"),
            Some((2, 2)),
            "no textual change still names an existing line"
        );
    }

    #[test]
    fn multi_line_edits_span_only_the_changed_block() {
        let old = "1\n2\n3\n4\n5\n6\n";
        let new = "1\n2\nTHREE\nFOUR\n5\n6\n";
        assert_eq!(changed_line_range(old, new), Some((3, 4)));
    }

    /// Line terminators are content: rewriting them changes those lines,
    /// so the span must cover them or evidence could be sidestepped by
    /// converting a file's endings.
    #[test]
    fn changing_only_line_endings_still_names_those_lines() {
        assert_eq!(
            changed_line_range("a\nb\nc\n", "a\r\nb\r\nc\n"),
            Some((1, 2)),
            "LF to CRLF on the first two lines"
        );
        assert_eq!(
            changed_line_range("a\nb\n", "a\nb"),
            Some((2, 2)),
            "dropping the final newline changes the last line"
        );
    }
}
