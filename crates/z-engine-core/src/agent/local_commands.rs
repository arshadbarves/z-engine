//! Commands the loop runs on the local machine rather than through the
//! model: lifecycle hooks from `[hooks]` in config.toml, and the `!`
//! shell passthrough. Both report their output as status notes, and
//! neither is ever allowed to end a turn.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;

use crate::tools::ToolCtx;

use super::events::Event;

/// Run a lifecycle hook (`[hooks]` in config.toml) with a hard timeout.
/// stdout becomes a status note; failures are reported but never fatal.
pub(super) async fn run_hook(
    hooks: &BTreeMap<String, String>,
    event: &str,
    root: &Path,
    ev_tx: &UnboundedSender<Event>,
) {
    let Some(cmd) = hooks.get(event) else {
        return;
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        crate::tools::shell_line(cmd)
            .current_dir(root)
            .env("ZENGINE_EVENT", event)
            .env("HARNESS_EVENT", event)
            .env("ZENGINE_PROJECT_ROOT", root)
            .env("HARNESS_PROJECT_ROOT", root)
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
        Ok(Err(e)) => {
            tracing::warn!(event, error = %e, "hook spawn failed");
        }
        Err(_) => {
            let _ = ev_tx.send(Event::StatusNote(format!(
                "[hook:{event}] timed out after 15s"
            )));
        }
    }
}

/// `!<cmd>` passthrough: run locally through the bash tool so output is
/// truncated/spilled consistently; never touches the model.
pub(super) async fn run_shell_passthrough(
    cmd: &str,
    bash: Arc<dyn crate::tools::Tool>,
    ctx: &ToolCtx,
    ev_tx: &UnboundedSender<Event>,
) {
    use serde_json::json;
    let input = json!({"command": cmd.to_string()});
    let out = bash.run(input, ctx).await;
    let text = match out {
        Ok(o) => o.result,
        Err(e) => format!("ERROR: {e}"),
    };
    for line in text.lines().take(40) {
        let _ = ev_tx.send(Event::StatusNote(format!("$ {line}")));
    }
}
