//! `lsp_diagnostics` tool plus the diagnostics rendering and the post-edit
//! hook that appends compiler-grade feedback to edit/write results
//! (spec §9 v0.8).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::lsp::DIAGNOSTICS_WAIT;
use crate::tools::{Tool, ToolCtx, ToolError, ToolOutput};

fn render_diagnostics(diags: &[Value]) -> String {
    use std::fmt::Write;
    if diags.is_empty() {
        return String::new();
    }
    let mut out = String::from("[lsp diagnostics]\n");
    for d in diags {
        let sev = match d["severity"].as_i64() {
            Some(1) => "E",
            Some(2) => "W",
            Some(3) => "I",
            Some(4) => "H",
            _ => "?",
        };
        let range = &d["range"]["start"];
        let line = range["line"].as_i64().map(|l| l + 1).unwrap_or(0);
        let msg = d["message"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(300)
            .collect::<String>();
        let code = d["code"].as_str().unwrap_or("");
        let _ = writeln!(out, "[lsp {sev}{code} line {line}] {msg}");
    }
    out
}

/// Post-edit hook: push fresh content to the server and wait briefly for
/// diagnostics. Returns text to append to the tool result.
async fn attach_lsp_diagnostics(ctx: &ToolCtx, abs: &Path) -> String {
    // Preferred: interactive server.
    if let Some(lsp) = &ctx.lsp {
        if let Ok(text) = tokio::fs::read_to_string(abs).await {
            if lsp.open_document(abs, &text).await.is_ok() {
                let diags = lsp.wait_diagnostics(abs, DIAGNOSTICS_WAIT).await;
                let rendered = render_diagnostics(&diags);
                if !rendered.is_empty() {
                    return rendered;
                }
            }
        }
    }
    // Deterministic compiler-grade feedback (spec section 9 v0.8):
    // cargo check JSON messages, filtered to the edited file.
    let root = ctx.project_root.clone();
    let outcome = tokio::task::spawn_blocking(move || crate::lsp::cargo_check::run(&root)).await;
    match outcome {
        Ok(Ok(diags)) => {
            let rel = abs
                .strip_prefix(&ctx.project_root)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| abs.to_string_lossy().into_owned());
            let mine = crate::lsp::cargo_check::render_for_file(&diags, &rel);
            render_diagnostics(&mine)
        }
        Ok(Err(e)) => format!("\n[lsp] cargo check failed: {e}"),
        Err(e) => format!("\n[lsp] cargo check task failed: {e}"),
    }
}

#[derive(Debug)]
pub struct DiagnosticsTool;

#[async_trait]
impl Tool for DiagnosticsTool {
    fn name(&self) -> &str {
        "lsp_diagnostics"
    }

    fn description(&self) -> &str {
        "Fetch current compiler-grade diagnostics (errors/warnings) for a \
         Rust file from rust-analyzer."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or(ToolError::InvalidInput {
                tool: "lsp_diagnostics",
                problem: "`path` required".into(),
            })?;
        let abs = ctx.resolve(Path::new(path));
        // Same deterministic backend as the post-edit hook.
        let rendered = attach_lsp_diagnostics(ctx, &abs).await;
        if rendered.is_empty() {
            Ok(ToolOutput::success(
                "no diagnostics".to_string(),
                "clean".to_string(),
            ))
        } else {
            Ok(ToolOutput::failure(rendered, "has diagnostics"))
        }
    }
}

/// If the edited file is Rust, wait briefly for rust-analyzer diagnostics
/// and append them to the tool-result content so errors feed back into the
/// loop automatically (spec section 9 v0.8).
pub(crate) async fn maybe_attach_diagnostics(
    tool_name: &str,
    ok: bool,
    input: &Value,
    ctx: &ToolCtx,
    result: &mut String,
) {
    if !(matches!(tool_name, "write_file" | "edit_file") && ok) || ctx.lsp.is_none() {
        return;
    }
    let Some(raw_path) = input.get("path").and_then(Value::as_str) else {
        return;
    };
    let abs: PathBuf = ctx.resolve(Path::new(raw_path));
    if abs.extension().and_then(|e| e.to_str()) != Some("rs") {
        return;
    }
    let diag = attach_lsp_diagnostics(ctx, &abs).await;
    if !diag.is_empty() {
        result.push('\n');
        result.push_str(&diag);
    }
}
