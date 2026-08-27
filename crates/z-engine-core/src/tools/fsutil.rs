use std::path::{Path, PathBuf};

use super::ToolCtx;

/// Character budget for a tool result entering the transcript.
pub const MAX_TOOL_OUTPUT_CHARS: usize = 16_000;

/// Crash-safe file replacement: write to a temp sibling, flush it to
/// disk, then atomically rename over the target. A crash mid-write can
/// never leave the target truncated or half-written (POSIX rename is
/// atomic; on Windows same-volume renames are best-effort but still far
/// safer than in-place truncation).
pub(crate) async fn atomic_write(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| std::io::Error::other("target has no parent directory"))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        ulid::Ulid::new()
    ));
    let write = async {
        let mut f = tokio::fs::File::create(&tmp).await?;
        tokio::io::AsyncWriteExt::write_all(&mut f, bytes).await?;
        f.sync_all().await?;
        std::io::Result::Ok(())
    };
    if let Err(e) = write.await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    match tokio::fs::rename(&tmp, target).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

/// Unified diff text between two versions of a file.
pub fn unified_diff(old: &str, new: &str, display_path: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(2)
        .header(&format!("a/{display_path}"), &format!("b/{display_path}"))
        .to_string()
}

/// Truncate `output` to fit the transcript budget, preserving head and
/// tail, and park the complete text in a temp file referenced inline.
pub fn truncate_with_tempfile(output: &str, ctx: &ToolCtx) -> String {
    if output.chars().count() <= MAX_TOOL_OUTPUT_CHARS {
        return output.to_string();
    }

    let path = next_tempfile_path(ctx);
    if let Err(e) = std::fs::write(&path, output) {
        tracing::warn!(%e, "failed writing full tool output tempfile");
        // Fall back to hard truncation without a pointer.
    }

    let total = output.chars().count();
    let budget = MAX_TOOL_OUTPUT_CHARS.saturating_sub(160); // room for marker
    let head = budget * 60 / 100;
    let tail = budget - head;

    let mut out = String::with_capacity(MAX_TOOL_OUTPUT_CHARS);
    out.extend(output.chars().take(head));
    let omitted = total - head - tail;
    out.push_str(&format!(
        "\n[...truncated {omitted} chars; full output: {}]\n",
        path.display()
    ));
    out.extend(output.chars().skip(total - tail));
    out
}

/// Write the full output to its own file even when under budget? No — only
/// truncation spills to disk. This helper just names spill files.
fn next_tempfile_path(ctx: &ToolCtx) -> PathBuf {
    let dir = ctx.tmp_dir.join("z-engine");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("out-{}.log", ulid::Ulid::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn ctx() -> ToolCtx {
        let tmp = tempfile::tempdir().unwrap();
        ToolCtx::new(
            tmp.path().to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tmp.path().to_path_buf(),
        )
    }

    #[test]
    fn short_output_passes_through_unmodified() {
        let c = ctx();
        assert_eq!(truncate_with_tempfile("hello", &c), "hello");
    }

    #[test]
    fn long_output_truncated_head_tail_with_marker_and_spill_file() {
        let c = ctx();
        let big: String = "x".repeat(50_000);
        let out = truncate_with_tempfile(&big, &c);

        assert!(out.len() < MAX_TOOL_OUTPUT_CHARS + 200);
        assert!(out.starts_with("xxxx"));
        assert!(out.ends_with("xxxx"));
        let marker_at = out.find("[...truncated ").unwrap();
        let path_start = out[marker_at..].find("/").map(|i| marker_at + i).unwrap();
        let path_end = out[path_start..].find(']').unwrap() + path_start;
        let spill = PathBuf::from(&out[path_start..path_end]);
        let full = std::fs::read_to_string(&spill).unwrap();
        assert_eq!(full.len(), 50_000);
    }

    #[test]
    fn multibyte_content_counted_by_chars_not_bytes() {
        let c = ctx();
        let big = "é".repeat(20_000); // 40k bytes, 20k chars > budget
        let out = truncate_with_tempfile(&big, &c);
        assert!(out.contains("[...truncated"));
    }
}
