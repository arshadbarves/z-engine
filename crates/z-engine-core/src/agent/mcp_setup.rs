//! Wiring external MCP tool servers into a run's registry.
//!
//! Separate from the command loop because it changes for its own reasons:
//! server lifecycle, tool discovery, and which runs may use external
//! tools at all.

use tokio::sync::mpsc::UnboundedSender;

use crate::tools::ToolRegistry;

use super::LoopConfig;
use super::events::Event;

/// Start each configured MCP server and register the tools it advertises.
///
/// Failures are logged and skipped: a broken server must not kill the
/// session.
///
/// Guarded runs register none of them. An MCP tool's effects are opaque
/// to this process, so it can neither be proven read-only nor routed
/// through the mutation gate; advertising one would leave an ungoverned
/// path to the working tree in a run whose whole premise is that every
/// mutation is authorized. The omission is reported rather than silent.
pub(super) async fn register_servers(
    cfg: &LoopConfig,
    registry: &mut ToolRegistry,
    ev_tx: &UnboundedSender<Event>,
) {
    if cfg.mcp_servers.is_empty() {
        return;
    }
    if cfg.guarded {
        let _ = ev_tx.send(Event::StatusNote(format!(
            "guarded mode: {} mcp server(s) not registered — external tools cannot be governed",
            cfg.mcp_servers.len()
        )));
        return;
    }
    for srv_cfg in &cfg.mcp_servers {
        let conn = crate::mcp::McpConnection::new(
            &srv_cfg.name,
            &srv_cfg.command,
            &srv_cfg.args,
            &cfg.project_root,
        );
        match conn.ensure().await {
            Err(e) => {
                tracing::warn!(server = %srv_cfg.name, error = %e, "mcp server failed to start");
            }
            Ok(()) => {
                for info in conn.list_tools().await {
                    registry.register(std::sync::Arc::new(crate::mcp::tool_adapter::McpTool {
                        conn: std::sync::Arc::new(conn.clone()),
                        info,
                    }));
                }
                let _ = ev_tx.send(Event::StatusNote(format!(
                    "registered mcp server '{}'",
                    srv_cfg.name
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with_server(guarded: bool) -> LoopConfig {
        let mut cfg = LoopConfig::new("m", "http://127.0.0.1:1/v1");
        cfg.guarded = guarded;
        cfg.mcp_servers = vec![crate::mcp::McpServerConfig {
            name: "fs".into(),
            command: "definitely-not-a-server".into(),
            args: vec![],
        }];
        cfg
    }

    /// A guarded run must not even try to start servers whose tools it
    /// could not govern, and must say why rather than silently dropping
    /// configured capability.
    #[tokio::test]
    async fn guarded_runs_register_no_external_tools_and_report_it() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();
        let before = registry.names().len();

        register_servers(&cfg_with_server(true), &mut registry, &tx).await;

        assert_eq!(registry.names().len(), before);
        let reported = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            reported.iter().any(|e| matches!(
                e,
                Event::StatusNote(m) if m.contains("not registered")
            )),
            "{reported:?}"
        );
    }

    #[tokio::test]
    async fn unguarded_runs_survive_a_server_that_cannot_start() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut registry = ToolRegistry::builtins();
        let before = registry.names().len();

        register_servers(&cfg_with_server(false), &mut registry, &tx).await;

        assert_eq!(registry.names().len(), before, "nothing to register");
    }
}
