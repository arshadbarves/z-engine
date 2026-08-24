//! `write_file` — full-file write gated by approval; returns a unified diff
//! of what changed. Overwriting an existing file requires a prior read.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;

use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile, unified_diff};

const PREVIEW_DIFF_CHARS: usize = 1_600;

#[derive(Debug)]
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Create or fully overwrite a text file. Overwriting an existing file \
         requires reading it first. The approval prompt shows the resulting \
         unified diff."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path relative to project root (or absolute)."},
                "content": {"type": "string", "description": "The complete new file content."}
            },
            "required": ["path", "content"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    fn approval_preview(&self, input: &Value, ctx: &ToolCtx) -> Option<String> {
        let path = input.get("path")?.as_str()?;
        let new = input.get("content")?.as_str()?;
        let resolved = ctx.resolve(Path::new(path));
        let display = rel(&resolved, &ctx.project_root);
        let old = std::fs::read_to_string(&resolved)
            .map(|s| s.into_bytes())
            .unwrap_or_default();
        let old_str = String::from_utf8_lossy(&old);
        Some(short_diff(&old_str, new, &display))
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "write_file",
            problem: "input must be an object".into(),
        })?;
        let raw_path =
            obj.get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidInput {
                    tool: "write_file",
                    problem: "`path` must be a string".into(),
                })?;
        let content =
            obj.get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidInput {
                    tool: "write_file",
                    problem: "`content` must be a string".into(),
                })?;

        let resolved = ctx.resolve(Path::new(raw_path));
        let disp = rel(&resolved, &ctx.project_root);

        let existed = resolved.exists();
        if existed {
            ctx.require_read_for_mutation("write_file", &resolved, true)?;
        }
        let old = if existed {
            String::from_utf8_lossy(
                &std::fs::read(&resolved)
                    .map_err(|e| ToolError::Failed(format!("read {}: {e}", resolved.display())))?,
            )
            .into_owned()
        } else {
            String::new()
        };

        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::Failed(format!("mkdir {}: {e}", parent.display())))?;
        }
        tokio::fs::write(&resolved, content)
            .await
            .map_err(|e| ToolError::Failed(format!("write {}: {e}", resolved.display())))?;
        ctx.note_read(&resolved);

        let diff = if existed {
            unified_diff(&old, content, &disp)
        } else {
            format!("new file {disp} ({} bytes)", content.len())
        };
        let body = format!("wrote {} to {disp}\n{diff}", content.len());
        let result = truncate_with_tempfile(&body, ctx);
        tracing::debug!(path = %disp, bytes = content.len(), existed, "write_file done");
        Ok(ToolOutput::success(result, format!("wrote {disp}")))
    }
}

pub(crate) fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// Diff clamped to the modal preview budget.
pub(crate) fn short_diff(old: &str, new: &str, display: &str) -> String {
    let d = unified_diff(old, new, display);
    let mut out: String = d.chars().take(PREVIEW_DIFF_CHARS).collect();
    if out.chars().count() == PREVIEW_DIFF_CHARS {
        out.push_str("\n… (diff truncated)");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    #[tokio::test]
    async fn creates_new_files_without_prior_read() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_in(tmp.path());
        let out = WriteFileTool
            .run(json!({"path": "sub/new.txt", "content": "hello\n"}), &ctx)
            .await
            .unwrap();
        assert!(out.ok);
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("sub/new.txt")).unwrap(),
            "hello\n"
        );
        assert!(out.result.contains("new file"));
    }

    #[tokio::test]
    async fn overwrite_without_prior_read_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "old\n").unwrap();
        let err = WriteFileTool
            .run(
                json!({"path": "a.txt", "content": "new"}),
                &ctx_in(tmp.path()),
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("without reading it first"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn overwrite_after_read_returns_unified_diff() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.txt");
        std::fs::write(&p, "line1\nline2\nline3\n").unwrap();
        let ctx = ctx_in(tmp.path());
        ctx.note_read(&p);

        let out = WriteFileTool
            .run(
                json!({"path": "b.txt", "content": "line1\nTWO\nline3\n"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("-line2"));
        assert!(out.result.contains("+TWO"));
        assert!(out.result.contains("@@")); // hunk header
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "line1\nTWO\nline3\n");
    }
}
