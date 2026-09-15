use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;

use super::events::Event;
use crate::tools::ToolCtx;

/// Existing lifecycle hooks are advisory; their opaque effects block S1 certification.
pub(super) async fn run_hook(
    hooks: &BTreeMap<String, String>,
    event: &str,
    root: &Path,
    ev_tx: &UnboundedSender<Event>,
) {
    let Some(cmd) = hooks.get(event) else { return };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        crate::tools::shell_line(cmd)
            .current_dir(root)
            .env("ZENGINE_EVENT", event)
            .env("HARNESS_EVENT", event)
            .env("ZENGINE_PROJECT_ROOT", root)
            .env("HARNESS_PROJECT_ROOT", root)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    match output {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !text.is_empty() {
                let _ = ev_tx.send(Event::StatusNote(format!("[hook:{event}] {text}")));
            }
        }
        Ok(Ok(out)) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let _ = ev_tx.send(Event::StatusNote(format!(
                "[hook:{event}] failed (exit {}): {}",
                out.status,
                err.chars().take(160).collect::<String>()
            )));
        }
        Ok(Err(error)) => {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "[hook:{event}] could not run: {error}"
            )));
        }
        Err(_) => {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "[hook:{event}] timed out after 15s"
            )));
        }
    }
}

pub(super) async fn run_shell_passthrough(
    cmd: &str,
    bash: Arc<dyn crate::tools::Tool>,
    ctx: &ToolCtx,
    ev_tx: &UnboundedSender<Event>,
) {
    let input = serde_json::json!({"command": cmd});
    let out = bash.run(input, ctx).await;
    let text = match out {
        Ok(out) => out.result,
        Err(error) => format!("ERROR: {error}"),
    };
    for line in text.lines().take(40) {
        let _ = ev_tx.send(Event::StatusNote(format!("$ {line}")));
    }
}
