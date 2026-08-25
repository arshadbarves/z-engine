//! Registry adapter: wraps an MCP server tool as a harness `Tool`.

use crate::mcp::{McpConnection, McpToolInfo};
use crate::tools::{Tool, ToolCtx, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct McpTool {
    pub conn: Arc<McpConnection>,
    pub info: McpToolInfo,
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("server", &self.conn.inner.name)
            .field("tool", &self.info.name)
            .finish()
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn description(&self) -> &str {
        if self.info.description.is_empty() {
            "(external MCP tool)"
        } else {
            &self.info.description
        }
    }

    fn parameters_schema(&self) -> Value {
        self.info.input_schema.clone()
    }

    // External tools may hold arbitrary server-side state; serialize them.
    fn concurrency_safe(&self) -> bool {
        false
    }

    async fn run(&self, input: Value, _ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        match self.conn.call_tool(&self.info.name, input).await {
            Ok(text) => {
                let summary: String = text.chars().take(80).collect();
                Ok(ToolOutput::success(text.clone(), summary))
            }
            Err(e) => Ok(ToolOutput::failure(
                format!("ERROR: {e}"),
                format!("mcp error: {e}"),
            )),
        }
    }
}
