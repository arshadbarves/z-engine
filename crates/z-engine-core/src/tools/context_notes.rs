//! `update_context_notes` — the meta-output pseudo-tool (spec §6.1).
//!
//! The model reports progress / decisions / needs_later each turn; entries
//! land in the L1 [`NotesStore`] which survives every compaction.
//! `droppable` entries quoting a `[harness:tool-output id=…]` marker elide
//! that transcript entry immediately.

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput};
use crate::context::notes::NotesInput;

#[derive(Debug)]
pub struct UpdateContextNotesTool;

#[async_trait]
impl Tool for UpdateContextNotesTool {
    fn name(&self) -> &str {
        "update_context_notes"
    }

    fn description(&self) -> &str {
        "Record session context that must survive compaction: current \
         `progress`, firm `decisions`, and `needs_later` reminders. Also \
         mark outputs you no longer need as `droppable`: quote the \
         `[harness:tool-output id=…]` marker shown above an old result to \
         have it dropped from context immediately."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "progress": {"type": "array", "items": {"type": "string"},
                    "description": "What has been done recently."},
                "decisions": {"type": "array", "items": {"type": "string"},
                    "description": "Firm decisions that constrain later work."},
                "needs_later": {"type": "array", "items": {"type": "string"},
                    "description": "Things to pick up later in this session."},
                "droppable": {"type": "array", "items": {"type": "string"},
                    "description": "References like '[harness:tool-output id=x]' whose content may be dropped."}
            },
            "required": []
        })
    }

    fn concurrency_safe(&self) -> bool {
        true
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let parsed: NotesInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: "update_context_notes",
                problem: e.to_string(),
            })?;
        let droppable_count = parsed.droppable.len();
        {
            let mut store = ctx
                .notes
                .lock()
                .map_err(|_| ToolError::Failed("notes lock poisoned".into()))?;
            store.merge(&parsed.progress, &parsed.decisions, &parsed.needs_later);
            store.mark_droppable(&parsed.droppable);
        }
        Ok(ToolOutput::success(
            format!("notes recorded ({} droppable refs)", droppable_count),
            "context notes updated".to_string(),
        ))
    }
}
