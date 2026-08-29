//! Compiler-grade diagnostics via `cargo check --message-format=json`.
//!
//! This is the deterministic backend behind the post-edit hook and
//! `lsp_diagnostics`: every Rust project harness serves already requires
//! cargo, so this always works — unlike interactive language servers whose
//! worker threads some environments refuse to spawn (see deviations.md).

use crate::lsp::batch::Diagnostic;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

/// Run `cargo check` on the workspace/package at `project_root`, returning
/// diagnostics whose primary span lands in a `.rs` source file.
pub fn run(project_root: &Path) -> Result<Vec<Diagnostic>, String> {
    let out = Command::new("cargo")
        .args(["check", "--message-format=json", "--all-targets"])
        .current_dir(project_root)
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;

    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

/// Parse `cargo --message-format=json` stdout into diagnostics.
///
/// Separate from [`run`] so callers that already own a bounded child
/// process (the guarded verification runner) can reuse this decoding
/// instead of spawning cargo a second way. Note that an empty result does
/// *not* mean success — a manifest error, for instance, emits no
/// compiler messages at all — so the exit status stays authoritative.
pub fn parse(stdout: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["reason"].as_str() != Some("compiler-message") {
            continue;
        }
        let msg = &v["message"];
        let level = msg["level"].as_str().unwrap_or("").to_lowercase();
        // Only surface errors/warnings; notes are noise for the loop.
        if level != "error" && level != "warning" {
            continue;
        }
        // Primary span = first span marked is_primary, else first span.
        let spans = msg["spans"].as_array();
        let primary = spans
            .and_then(|spans| {
                spans
                    .iter()
                    .find(|s| s["is_primary"].as_bool() == Some(true))
                    .or_else(|| spans.first())
            })
            .cloned()
            .unwrap_or(Value::Null);
        let file = primary["file_name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if file.is_empty() || !file.ends_with(".rs") {
            continue;
        }
        let line = primary["line_start"].as_u64().unwrap_or(0) as usize;
        let code = msg["code"]["code"].as_str().unwrap_or("-").to_string();
        let mut text = msg["message"].as_str().unwrap_or("").to_string();

        // Include labeled children (helpful "expected X, found Y" notes).
        for child in msg["children"].as_array().unwrap_or(&vec![]).iter() {
            let cmsg = child["message"].as_str().unwrap_or("");
            if !cmsg.is_empty()
                && child["children"]
                    .as_array()
                    .map(|a| a.is_empty())
                    .unwrap_or(true)
            {
                text.push_str("; ");
                text.push_str(cmsg);
            }
        }

        diags.push(Diagnostic {
            severity: level,
            code,
            file,
            line,
            message: text,
        });
    }
    diags
}

/// Render into the same shape as the LSP renderer consumers expect.
pub fn render_for_file(diags: &[Diagnostic], rel_file: &str) -> Vec<Value> {
    diags
        .iter()
        .filter(|d| d.file.replace('\\', "/").ends_with(rel_file))
        .map(|d| {
            serde_json::json!({
                "severity": if d.severity == "error" { 1 } else { 2 },
                "code": d.code,
                "range": {"start": {"line": d.line.saturating_sub(1)}},
                "message": format!("{} ({})", d.message, d.file)
            })
        })
        .collect()
}
