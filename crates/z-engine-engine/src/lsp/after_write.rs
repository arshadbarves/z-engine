//! Error diagnostics for files the model just wrote, collected with a
//! bounded wait. Only fresh diagnostics count: a server that has not
//! analyzed the new content in time adds nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use z_engine_context::{render_template, wrap_reminder};
use z_engine_host::relative_display;
use z_engine_integrations::{Diagnostic, FileDiagnostics, IntegrationError, Severity};
use z_engine_prompts::reminders::LSP_ERRORS;

use super::worker::LspWorker;

/// Longest wait for all files together.
const WAIT: Duration = Duration::from_secs(5);
/// Error lines quoted per file.
const MAX_LINES: usize = 20;

/// A `<system-reminder>` per written file that has errors.
pub(crate) async fn error_notes(
    worker: Arc<LspWorker>,
    root: &Path,
    files: Vec<PathBuf>,
) -> HashMap<PathBuf, String> {
    let collect = worker.run(move |manager| async move {
        let checks = files.into_iter().map(|file| {
            let manager = Arc::clone(&manager);
            async move {
                let report = manager.diagnostics(&file).await;
                (file, report)
            }
        });
        futures::future::join_all(checks).await
    });
    let reports: Vec<(PathBuf, Result<FileDiagnostics, IntegrationError>)> =
        match tokio::time::timeout(WAIT, collect).await {
            Ok(Some(reports)) => reports,
            Ok(None) => return HashMap::new(),
            Err(_) => {
                tracing::debug!("post-edit diagnostics timed out");
                return HashMap::new();
            }
        };
    reports
        .into_iter()
        .filter_map(|(file, report)| {
            let report = report
                .map_err(|error| tracing::debug!(%error, file = %file.display(), "no diagnostics"))
                .ok()
                .filter(|report| report.fresh)?;
            let errors: Vec<&Diagnostic> = report
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .collect();
            let note = note(root, &file, &errors)?;
            Some((file, note))
        })
        .collect()
}

fn note(root: &Path, file: &Path, errors: &[&Diagnostic]) -> Option<String> {
    if errors.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = errors
        .iter()
        .take(MAX_LINES)
        .map(|diagnostic| {
            let code = diagnostic
                .code
                .as_deref()
                .map(|code| format!(" [{code}]"))
                .unwrap_or_default();
            format!(
                "{}:{}:{}: error: {}{code}",
                relative_display(root, &diagnostic.path),
                diagnostic.line,
                diagnostic.column,
                diagnostic.message.trim().replace('\n', " ")
            )
        })
        .collect();
    if errors.len() > MAX_LINES {
        lines.push(format!("... and {} more errors", errors.len() - MAX_LINES));
    }
    let shown = relative_display(root, file);
    Some(wrap_reminder(&render_template(
        LSP_ERRORS,
        &[("file", &shown), ("diagnostics", &lines.join("\n"))],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(line: u32) -> Diagnostic {
        Diagnostic {
            path: PathBuf::from("/p/src/lib.rs"),
            line,
            column: 3,
            end_line: line,
            end_column: 4,
            severity: Severity::Error,
            message: "bad\nthing".into(),
            code: Some("E1".into()),
            source: None,
        }
    }

    #[test]
    fn notes_quote_errors_with_a_cap() {
        let root = Path::new("/p");
        let file = Path::new("/p/src/lib.rs");
        assert!(note(root, file, &[]).is_none());
        let many: Vec<Diagnostic> = (1..=25).map(error).collect();
        let refs: Vec<&Diagnostic> = many.iter().collect();
        let text = note(root, file, &refs).unwrap();
        assert!(text.starts_with("<system-reminder>"));
        assert!(text.contains("errors in src/lib.rs"));
        assert!(text.contains("src/lib.rs:1:3: error: bad thing [E1]"));
        assert!(text.contains("... and 5 more errors"));
        assert!(!text.contains("src/lib.rs:21:3"));
    }
}
