//! LSP-backed tools: `go_to_definition`, `find_references`,
//! `lsp_diagnostics`, plus the post-edit diagnostics attachment used by the
//! loop's feedback hook (spec §9 v0.8).
//!
//! All tools degrade gracefully: when no LSP server is available for the
//! project they return a model-visible note instead of failing hard.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput};
use crate::lsp::DIAGNOSTICS_WAIT;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn decode_uri(uri: &str) -> String {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let mut out = String::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(b as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn position(line_1based: u64, column_1based: u64) -> Value {
    json!({
        "line": line_1based.saturating_sub(1),
        "character": column_1based.saturating_sub(1)
    })
}

fn text_document(path: &Path) -> Value {
    let uri = crate::lsp::percent_encode_path_public(path);
    json!({"uri": uri})
}

/// Extract a flat list of locations from a definition/references response.
fn locations_of(result: &Value) -> Vec<(String, usize, usize)> {
    let arr = match result {
        Value::Null => return Vec::new(),
        Value::Array(a) => a.clone(),
        v @ Value::Object(_) => vec![v.clone()],
        _ => return Vec::new(),
    };
    arr.iter()
        .filter_map(|loc| {
            let uri = loc["uri"].as_str()?;
            let line = loc["range"]["start"]["line"].as_u64()? + 1;
            let col = loc["range"]["start"]["character"].as_u64()? + 1;
            Some((decode_uri(uri), line as usize, col as usize))
        })
        .collect()
}

pub(crate) fn render_diagnostics(diags: &[Value]) -> String {
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
pub(crate) async fn attach_lsp_diagnostics(ctx: &ToolCtx, abs: &Path) -> String {
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

async fn request_locations(
    ctx: &ToolCtx,
    method: &str,
    path: &str,
    line: u64,
    column: u64,
    references: bool,
) -> Result<ToolOutput, ToolError> {
    let abs = ctx.resolve(Path::new(path));
    if !abs.exists() {
        return Ok(ToolOutput::failure(
            format!("ERROR: file not found: {}", abs.display()),
            "missing file",
        ));
    }

    // Preferred: live language server.
    if let Some(lsp) = &ctx.lsp {
        if let Ok(text) = tokio::fs::read_to_string(&abs).await {
            let _ = lsp.open_document(&abs, &text).await;
        }
        let params = json!({
            "textDocument": text_document(&abs),
            "position": position(line, column),
            "context": {"includeDeclaration": false}
        });
        if let Ok(v) = lsp.request(method, params).await {
            let locs = locations_of(&v);
            if !locs.is_empty() {
                let body: Vec<String> = locs
                    .iter()
                    .take(50)
                    .map(|(p, l, c)| format!("{p}:{l}:{c}"))
                    .collect();
                return Ok(ToolOutput::success(
                    body.join("\n"),
                    format!("{} hit(s) [lsp]", locs.len()),
                ));
            }
        }
    }

    // Fallback: outline-based definition / grep-based references.
    if !references {
        if let Some(name) =
            crate::context::repo_map::identifier_at(&abs, line as usize, column as usize)
        {
            let (outlines, _corpus) =
                crate::context::repo_map::generate(&ctx.project_root, 400, 200_000);
            for (fpath, symbols) in &outlines {
                if let Some(sym) = symbols.iter().find(|s| {
                    s.name == name && matches!(s.kind, "fn" | "struct" | "enum" | "trait" | "type")
                }) {
                    return Ok(ToolOutput::success(
                        format!("{}:{} {} {}", fpath.display(), sym.line, sym.kind, sym.name),
                        format!("definition of {name}"),
                    ));
                }
            }
            return Ok(ToolOutput::failure(
                format!("no definition of `{name}` found in project outlines"),
                "not found",
            ));
        }
        return Ok(ToolOutput::failure(
            "ERROR: no identifier at position".to_string(),
            "none".to_string(),
        ));
    }

    // References fallback: word-boundary grep minus the queried position.
    if let Some(name) =
        crate::context::repo_map::identifier_at(&abs, line as usize, column as usize)
    {
        let pattern = format!("\\b{name}\\b");
        if let Ok(out) = crate::tools::grep::GrepTool
            .run(json!({"pattern": pattern, "cap": 100}), ctx)
            .await
        {
            let mut lines: Vec<String> = out.result.lines().map(str::to_string).collect();
            lines.retain(|l| l.contains(':'));
            return Ok(ToolOutput::success(
                format!(
                    "{} reference(s) to `{name}`\n{}",
                    lines.len(),
                    lines.join("\n")
                ),
                format!("{} refs", lines.len()),
            ));
        }
    }
    Ok(ToolOutput::failure(
        "ERROR: could not resolve symbol".to_string(),
        "failed".to_string(),
    ))
}

#[allow(dead_code)]
async fn unused_request_locations(
    ctx: &ToolCtx,
    method: &str,
    path: &str,
    line: u64,
    column: u64,
    references: bool,
) -> Result<ToolOutput, ToolError> {
    let Some(lsp) = &ctx.lsp else {
        return Ok(ToolOutput::failure(
            "ERROR: no language server available for this project",
            "lsp unavailable",
        ));
    };
    let abs = ctx.resolve(Path::new(path));
    if !abs.exists() {
        return Ok(ToolOutput::failure(
            format!("ERROR: file not found: {}", abs.display()),
            "missing file",
        ));
    }
    // Make sure the server has current content before resolving positions.
    if let Ok(text) = tokio::fs::read_to_string(&abs).await {
        let _ = lsp.open_document(&abs, &text).await;
    }

    let params = json!({
        "textDocument": text_document(&abs),
        "position": position(line, column),
        "context": {"includeDeclaration": false}
    });
    let method = if references {
        "textDocument/references"
    } else {
        method
    };

    let result = lsp.request(method, params).await;
    match result {
        Err(e) => Ok(ToolOutput::failure(format!("ERROR: {e}"), "lsp error")),
        Ok(v) => {
            let locs = locations_of(&v);
            if locs.is_empty() {
                return Ok(ToolOutput::success(
                    "no locations found".to_string(),
                    "no locations".to_string(),
                ));
            }
            let body: Vec<String> = locs
                .iter()
                .take(50)
                .map(|(p, l, c)| format!("{p}:{l}:{c}"))
                .collect();
            Ok(ToolOutput::success(
                body.join("\n"),
                format!("{} hit(s)", locs.len()),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// tools
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct GoToDefinitionTool;

#[async_trait]
impl Tool for GoToDefinitionTool {
    fn name(&self) -> &str {
        "go_to_definition"
    }

    fn description(&self) -> &str {
        "Jump to the definition of the symbol at a 1-based line/column in a \
         Rust file (uses rust-analyzer)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "line": {"type": "integer", "description": "1-based line of the symbol occurrence."},
                "column": {"type": "integer", "description": "1-based column of the symbol occurrence."}
            },
            "required": ["path", "line", "column"]
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input
            .as_object()
            .ok_or(ToolError::Failed("bad input".into()))?;
        let path = obj.get("path").and_then(Value::as_str).unwrap_or_default();
        let line = obj.get("line").and_then(Value::as_u64).unwrap_or(1);
        let column = obj.get("column").and_then(Value::as_u64).unwrap_or(1);
        request_locations(ctx, "textDocument/definition", path, line, column, false)
            .await
            .map(|mut o| {
                o.summary = format!("definition: {}", o.summary);
                o
            })
    }
}

#[derive(Debug)]
pub struct FindReferencesTool;

#[async_trait]
impl Tool for FindReferencesTool {
    fn name(&self) -> &str {
        "find_references"
    }

    fn description(&self) -> &str {
        "Find all references to the symbol at a 1-based line/column in a \
         Rust file (uses rust-analyzer)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "line": {"type": "integer"},
                "column": {"type": "integer"}
            },
            "required": ["path", "line", "column"]
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input
            .as_object()
            .ok_or(ToolError::Failed("bad input".into()))?;
        let path = obj.get("path").and_then(Value::as_str).unwrap_or_default();
        let line = obj.get("line").and_then(Value::as_u64).unwrap_or(1);
        let column = obj.get("column").and_then(Value::as_u64).unwrap_or(1);
        request_locations(ctx, "textDocument/references", path, line, column, true).await
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

// ---------------------------------------------------------------------------
// post-edit hook used by the loop
// ---------------------------------------------------------------------------

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
