use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput};
use crate::verification::{CheckOutcome, CheckSpec, TaskStatus, blocked_evidence, run_check};

#[derive(Debug)]
pub struct RunVerificationTool;

#[async_trait]
impl Tool for RunVerificationTool {
    fn name(&self) -> &str {
        "run_verification"
    }

    fn description(&self) -> &str {
        "Run a typed Cargo check and record durable evidence."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type":"object", "additionalProperties":false,
            "properties": {
                "kind":{"type":"string","enum":["cargo_test","cargo_build"]},
                "package":{"type":["string","null"]},
                "filter":{"type":["string","null"]}
            },
            "required":["kind"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    fn approval_preview(&self, input: &Value, _ctx: &ToolCtx) -> Option<String> {
        serde_json::from_value::<CheckSpec>(input.clone())
            .ok()
            .and_then(|spec| spec.command().ok())
            .map(|command| command.join(" "))
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let invalid = |error: String| ToolError::InvalidInput {
            tool: "run_verification",
            problem: error,
        };
        let spec: CheckSpec = serde_json::from_value(input).map_err(|e| invalid(e.to_string()))?;
        spec.validate().map_err(|e| invalid(e.to_string()))?;
        let task_id = ctx
            .task
            .lock()
            .map_err(|_| ToolError::Failed("task lock poisoned".into()))?
            .as_ref()
            .ok_or_else(|| ToolError::Failed("no active task".into()))?
            .task_id
            .clone();
        let directory = ctx
            .evidence_dir
            .as_ref()
            .ok_or_else(|| ToolError::Failed("no durable evidence directory".into()))?;
        let evidence = match run_check(
            &ctx.project_root,
            spec.clone(),
            directory,
            ctx.abort.clone(),
        )
        .await
        {
            Ok(evidence) => evidence,
            Err(error) => blocked_evidence(&ctx.project_root, spec, error.to_string())
                .map_err(|e| ToolError::Failed(e.to_string()))?,
        };
        let mut guard = ctx
            .task
            .lock()
            .map_err(|_| ToolError::Failed("task lock poisoned".into()))?;
        let report = guard
            .as_mut()
            .filter(|report| report.task_id == task_id)
            .ok_or_else(|| ToolError::Failed("active task changed during verification".into()))?;
        report.checks.push(evidence.clone());
        report.assessment = None;
        report.status = TaskStatus::NeedsVerification;
        let output = json!({"evidenceId":evidence.id, "evidence":evidence}).to_string();
        Ok(if evidence.outcome == CheckOutcome::Passed {
            ToolOutput::success(output, evidence.summary)
        } else {
            ToolOutput::failure(output, evidence.summary)
        })
    }
}
