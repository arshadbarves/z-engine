//! Shared LSP plumbing for the location-based tools: URI/position
//! encoding, response parsing, and the location request pipeline shared by
//! go-to-definition and find-references.

use std::path::Path;

use serde_json::{Value, json};

use crate::tools::{Tool, ToolCtx, ToolError, ToolOutput};

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

pub(super) async fn request_locations(
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
