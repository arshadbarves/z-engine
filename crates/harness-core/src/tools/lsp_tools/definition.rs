//! `go_to_definition` — jump to the definition of a symbol via the LSP.

use async_trait::async_trait;
use serde_json::{Value, json};

use super::helpers::request_locations;
use crate::tools::{Tool, ToolCtx, ToolError, ToolOutput};

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
