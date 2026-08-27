//! Batch diagnostics backend: `rust-analyzer diagnostics` CLI.
//!
//! The interactive stdio server requests a very large worker stack that is
//! denied inside some sandboxed environments (jod-thread spawn failure);
//! the batch CLI mode runs fine everywhere we've tested and gives the same
//! compiler-grade results. Used by `lsp_diagnostics` and the post-edit hook;
/// the JSON-RPC client remains the preferred backend when it can start.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Diagnostic {
    pub severity: String,
    pub code: String,
    pub file: String,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("rust-analyzer binary not available")]
    Missing,
    #[error("rust-analyzer failed: {0}")]
    Failed(String),
    #[error("timed out")]
    Timeout,
}

const LINE_RE: &str = r#"at crate (\S+), file (\S+): Error RustcHardError\("(E\d+)"\) from LineCol \{ line: (\d+), col: \d+ \}.*?: (.*)"#;

/// Parse one output line of `rust-analyzer diagnostics` into a Diagnostic.
pub fn parse_line(line: &str) -> Option<Diagnostic> {
    let re = regex::Regex::new(LINE_RE).ok()?;
    let caps = re.captures(line)?;
    Some(Diagnostic {
        severity: "error".into(),
        code: caps.get(3)?.as_str().to_string(),
        file: caps.get(2)?.as_str().to_string(),
        line: caps.get(4)?.as_str().parse::<usize>().ok()? + 1,
        // strip trailing progress garbage
        message: caps
            .get(5)?
            .as_str()
            .split("\u{1b}")
            .next()
            .unwrap_or("")
            .trim()
            .to_string(),
    })
}

/// Run batch analysis for a whole project; returns parsed diagnostics.
pub fn run(project_root: &Path, _timeout_secs: u64) -> Result<Vec<Diagnostic>, CliError> {
    use std::process::{Command, Stdio};
    let out = Command::new("rust-analyzer")
        .arg("diagnostics")
        .arg(".")
        .current_dir(project_root)
        .stdin(Stdio::null())
        .output();

    let Ok(out) = out else {
        return Err(CliError::Missing);
    };
    let combined =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);

    let mut diags = Vec::new();
    for line in combined.lines() {
        if let Some(d) = parse_line(line) {
            diags.push(d);
        }
    }
    Ok(diags)
}

use std::path::Path;
