//! `read_file` — line-numbered file reading with offset/limit windows and
//! binary detection (spec §7, ships v0.1).

use std::path::Path;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use super::{Tool, ToolCtx, ToolError, ToolOutput};

/// Default and hard caps on lines returned per call — context protection.
const DEFAULT_LIMIT: usize = 2_000;
const MAX_LIMIT: usize = 5_000;
/// NUL byte in the first 8 KiB ⇒ treat as binary.
const BINARY_SNIFF_BYTES: usize = 8_192;

#[derive(Debug)]
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a text file with line numbers. Supports `offset` (1-based first \
         line) and `limit` (max lines, default 2000). Binary files are \
         detected and not dumped."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path, relative to the project root (or absolute)."
                },
                "offset": {
                    "type": "integer",
                    "description": "1-based line number to start from (default 1)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to return (default 2000, max 5000)."
                }
            },
            "required": ["path"]
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "read_file",
            problem: "input must be an object".into(),
        })?;
        let raw_path = obj
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "read_file",
                problem: "`path` must be a non-empty string".into(),
            })?;
        let offset = obj
            .get("offset")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as usize;
        let limit = obj
            .get("limit")
            .and_then(Value::as_u64)
            .map(|l| l as usize)
            .unwrap_or(DEFAULT_LIMIT)
            .clamp(1, MAX_LIMIT);

        let path = ctx.resolve(Path::new(raw_path));
        let display_path = display_rel(&path, &ctx.project_root);

        // Binary sniff before committing to a full read.
        match sniff_binary(&path).await {
            Ok(true) => {
                ctx.note_read(&path);
                let size = std::fs::metadata(&path)
                    .map(|m| m.len())
                    .unwrap_or_default();
                let msg = format!("[binary file; {size} bytes; not displayed]");
                return Ok(ToolOutput::success(
                    msg,
                    format!("read_file: {display_path} (binary)"),
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ToolOutput::failure(
                    format!("ERROR: file not found: {}", path.display()),
                    format!("read_file: missing {display_path}"),
                ));
            }
            Err(e) => {
                return Ok(ToolOutput::failure(
                    format!("ERROR: cannot read {}: {e}", path.display()),
                    format!("read_file: error {display_path}"),
                ));
            }
            Ok(false) => {}
        }

        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|e| ToolError::Failed(format!("open {}: {e}", path.display())))?;
        let mut lines = BufReader::new(file).lines();

        let mut body = String::new();
        let mut number = 0usize;
        let mut shown_first = 0usize;
        let mut shown_last = 0usize;
        let mut truncated_by_limit = false;
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|e| ToolError::Failed(format!("read {}: {e}", path.display())))?
        {
            number += 1;
            if number < offset {
                continue;
            }
            if number >= offset + limit {
                truncated_by_limit = true;
                break;
            }
            if shown_first == 0 {
                shown_first = number;
            }
            shown_last = number;
            body.push_str(&format!("{number:>4} │ {line}\n"));
        }

        if number == 0 {
            let msg = "[empty file]\n".to_string();
            return Ok(ToolOutput::success(
                msg,
                format!("read_file: {display_path} (empty)"),
            ));
        }
        ctx.note_read(&path);
        if shown_first == 0 {
            return Ok(ToolOutput::failure(
                format!("ERROR: offset {offset} is past end of file ({number} lines total)"),
                format!("read_file: {display_path} (bad offset)"),
            ));
        }

        if truncated_by_limit {
            body.push_str(&format!(
                "[showing lines {shown_first}–{shown_last}; more lines follow — call again with a larger offset]"
            ));
        }

        let summary = format!("read_file: {display_path} (lines {shown_first}–{shown_last})");
        Ok(ToolOutput::success(body, summary))
    }
}

async fn sniff_binary(path: &Path) -> std::io::Result<bool> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut probe = vec![0u8; BINARY_SNIFF_BYTES];
    let n = f.read(&mut probe).await?;
    Ok(probe[..n].contains(&0u8))
}

fn display_rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn ctx_in(dir: &std::path::Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    #[tokio::test]
    async fn reads_with_line_numbers() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.txt");
        std::fs::write(&p, "alpha\nbeta\ngamma\n").unwrap();
        let out = ReadFileTool
            .run(json!({"path": "a.txt"}), &ctx_in(tmp.path()))
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("   1 │ alpha"));
        assert!(out.result.contains("   3 │ gamma"));
        assert!(out.summary.contains("lines 1–3"));
    }

    #[tokio::test]
    async fn offset_and_limit_window() {
        let tmp = tempfile::tempdir().unwrap();
        let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
        std::fs::write(tmp.path().join("b.txt"), content).unwrap();
        let out = ReadFileTool
            .run(
                json!({"path": "b.txt", "offset": 4, "limit": 3}),
                &ctx_in(tmp.path()),
            )
            .await
            .unwrap();
        assert!(out.result.contains("   4 │ line4"));
        assert!(out.result.contains("   6 │ line6"));
        assert!(!out.result.contains("line7"));
        assert!(out.result.contains("more lines follow"));
    }

    #[tokio::test]
    async fn offset_past_end_is_a_model_visible_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("c.txt"), "one\n").unwrap();
        let out = ReadFileTool
            .run(json!({"path": "c.txt", "offset": 99}), &ctx_in(tmp.path()))
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.result.contains("past end of file"));
    }

    #[tokio::test]
    async fn missing_file_reports_error_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let out = ReadFileTool
            .run(json!({"path": "nope/missing.txt"}), &ctx_in(tmp.path()))
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.result.contains("file not found"));
    }

    #[tokio::test]
    async fn binary_files_are_not_dumped() {
        let tmp = tempfile::tempdir().unwrap();
        let bytes: Vec<u8> = [0x89u8, b'P', b'N', b'G', 0u8, 1, 2, 3].to_vec();
        std::fs::write(tmp.path().join("img.bin"), &bytes).unwrap();
        let out = ReadFileTool
            .run(json!({"path": "img.bin"}), &ctx_in(tmp.path()))
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("[binary file;"));
        assert!(!out.result.contains("\u{0}"));
    }

    #[tokio::test]
    async fn empty_file_and_absolute_paths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("e.txt"), "").unwrap();
        let ctx = ctx_in(tmp.path());
        let out = ReadFileTool
            .run(json!({"path": tmp.path().join("e.txt")}), &ctx)
            .await
            .unwrap();
        assert!(out.result.contains("[empty file]"));

        let err = ReadFileTool.run(json!({}), &ctx).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput { .. }));
    }
}
