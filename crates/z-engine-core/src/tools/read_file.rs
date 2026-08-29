//! `read_file` — line-numbered file reading with offset/limit windows and
//! binary detection (spec §7, ships v0.1).

use std::path::Path;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::read_file_evidence::capture_read_evidence;
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

        // Single read of the file's bytes: binary detection, the empty-file
        // check, the displayed window, the evidence range blob, and the
        // full-file hash all derive from this one snapshot. A concurrent
        // write between two *separate* reads could otherwise make evidence
        // describe bytes the model never actually saw in the displayed
        // output — reading once and reusing the same buffer for everything
        // makes that impossible by construction.
        let full = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
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
        };

        if is_binary(&full) {
            ctx.note_read(&path);
            let msg = format!("[binary file; {} bytes; not displayed]", full.len());
            return Ok(ToolOutput::success(
                msg,
                format!("read_file: {display_path} (binary)"),
            ));
        }

        if full.is_empty() {
            ctx.note_read(&path);
            let evidence_id = capture_read_evidence(ctx, &path, &full, None)?;
            let mut msg = "[empty file]\n".to_string();
            if let Some(id) = &evidence_id {
                msg.push_str(&format!("[evidence: {id}]"));
            }
            let mut out = ToolOutput::success(msg, format!("read_file: {display_path} (empty)"));
            out.evidence_ids = evidence_id.into_iter().collect();
            return Ok(out);
        }

        let text = std::str::from_utf8(&full).map_err(|e| {
            ToolError::Failed(format!("read {}: invalid UTF-8: {e}", path.display()))
        })?;

        let mut body = String::new();
        let mut number = 0usize;
        let mut shown_first = 0usize;
        let mut shown_last = 0usize;
        let mut truncated_by_limit = false;
        for line in text.lines() {
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

        let evidence_id =
            capture_read_evidence(ctx, &path, &full, Some((shown_first, shown_last)))?;
        if let Some(id) = &evidence_id {
            body.push_str(&format!("\n[evidence: {id}]"));
        }

        let summary = format!("read_file: {display_path} (lines {shown_first}–{shown_last})");
        let mut out = ToolOutput::success(body, summary);
        out.evidence_ids = evidence_id.into_iter().collect();
        Ok(out)
    }
}

/// NUL byte anywhere in the first [`BINARY_SNIFF_BYTES`] of `bytes` ⇒ treat
/// as binary. Operates on the already-read snapshot (no separate file
/// open/peek), so binary detection can never race against the same read
/// used for display/evidence.
fn is_binary(bytes: &[u8]) -> bool {
    let probe_len = bytes.len().min(BINARY_SNIFF_BYTES);
    bytes[..probe_len].contains(&0u8)
}

fn display_rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
