//! Conservative effect tracking until tools expose richer effect declarations.

use serde_json::Value;

use crate::perms::PolicyEngine;
use crate::tools::ToolCtx;

use super::task_completion::{self, TaskLifecycleError};

pub(super) fn before_call(
    name: &str,
    input: &Value,
    ctx: &ToolCtx,
) -> Result<(), TaskLifecycleError> {
    match name {
        "run_verification" | "assess_completion" => {}
        "read_file"
        | "glob"
        | "grep"
        | "inspect_project"
        | "go_to_definition"
        | "find_references"
        | "lsp_diagnostics"
        | "task"
        | "update_context_notes" => {}
        "write_file" | "edit_file" => {
            task_completion::invalidate(ctx)?;
            if input
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| ctx.is_outside_root(std::path::Path::new(path)))
            {
                task_completion::block(
                    ctx,
                    "An edit outside the workspace is not covered by workspace verification."
                        .into(),
                )?;
            }
        }
        "bash"
            if input
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(PolicyEngine::command_is_read_only) => {}
        _ => {
            task_completion::invalidate(ctx)?;
            task_completion::block(
                ctx,
                format!(
                    "The effects of {name} are not bounded to the verified workspace; S1 cannot certify this task."
                ),
            )?;
        }
    }
    Ok(())
}

pub(super) fn denied(name: &str, ctx: &ToolCtx) -> Result<(), TaskLifecycleError> {
    task_completion::block(ctx, format!("A requested {name} operation was denied."))
}
