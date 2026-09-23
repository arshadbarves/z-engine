//! The session's tool registry: built-in tools (the `Agent` description
//! listing the current agent types) followed by the MCP catalog.

use std::sync::Arc;

use z_engine_tools::ToolRegistry;

use crate::session::SessionCore;
use crate::sync::write;

/// Rebuilds the registry after agent types or MCP tools changed.
pub(crate) fn rebuild_tools(core: &SessionCore) {
    let mut registry = ToolRegistry::builtin(core.agents.registry().cards());
    core.mcp.catalog().register(&mut registry);
    *write(&core.tools) = Arc::new(registry);
}
