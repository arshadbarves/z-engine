//! `find_references` — find all references to a symbol via the LSP.

use async_trait::async_trait;
use serde_json::{Value, json};

use super::helpers::request_locations;
use crate::tools::{Tool, ToolCtx, ToolError, ToolOutput};

#[derive(Debug)]
pub struct FindReferencesTool;

#[async_trait]
impl Tool for FindReferencesTool {
    fn name(&self) -> &str {
        "find_references"
    }

    fn description(&self) -> &str {
        "Find all references to the symbol at a 1-based line/column in a \
         Rust file (uses rust-analyzer)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "line": {"type": "integer"},
                "column": {"type": "integer"}
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
        request_locations(ctx, "textDocument/references", path, line, column, true).await
    }
}
