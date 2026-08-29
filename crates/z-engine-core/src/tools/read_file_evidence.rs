//! Evidence capture for `read_file` (spec Task 3): turns one successful,
//! bounded text read into a durable, revision-scoped [`EvidenceRecord`]
//! when a [`super::EvidenceStore`] is attached to the [`ToolCtx`]. Kept
//! separate from the `read_file` tool implementation itself so the I/O
//! and windowing logic there doesn't have to know how evidence is shaped.

use std::path::Path;

use super::{ToolCtx, ToolError};

/// Record the just-returned line range as evidence, when an evidence
/// recorder is attached. Only reachable from the successful, non-binary
/// text path in `ReadFileTool::run` — binary or failed reads never call
/// this, so they can never become edit-authorizing evidence.
pub(super) async fn capture_read_evidence(
    ctx: &ToolCtx,
    path: &Path,
    first_line: usize,
    last_line: usize,
) -> Result<Option<String>, ToolError> {
    if ctx.evidence.is_none() {
        return Ok(None);
    }
    // The ledger's `file_hash` covers the *whole* file (for staleness
    // detection), which the streaming line-by-line pass in `read_file`
    // may not have read to EOF when the limit truncated it — re-read in
    // full, but only when evidence capture is actually enabled.
    let full = tokio::fs::read(path)
        .await
        .map_err(|e| ToolError::Failed(format!("hashing {}: {e}", path.display())))?;
    let range = extract_line_range(&full, first_line, last_line);
    ctx.record_read_evidence(
        path,
        Some((first_line as u32, last_line as u32)),
        &full,
        &range,
    )
}

/// Raw bytes of the inclusive 1-based `[first, last]` line range, joined
/// with `\n` and without the display's line-number prefixes — the literal
/// content actually read, stored as this call's evidence blob.
fn extract_line_range(bytes: &[u8], first: usize, last: usize) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .lines()
        .skip(first.saturating_sub(1))
        .take(last + 1 - first)
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_line_range_returns_only_requested_lines_without_prefixes() {
        let content = (1..=10).map(|i| format!("line{i}\n")).collect::<String>();
        let range = extract_line_range(content.as_bytes(), 4, 6);
        assert_eq!(String::from_utf8(range).unwrap(), "line4\nline5\nline6");
    }

    #[test]
    fn extract_line_range_single_line() {
        let content = "only one line\n";
        let range = extract_line_range(content.as_bytes(), 1, 1);
        assert_eq!(String::from_utf8(range).unwrap(), "only one line");
    }
}
