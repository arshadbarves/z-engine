use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput};
use crate::verification::{CompletionAssessment, TaskStatus, WorkspaceSnapshot, assess};

#[derive(Debug)]
pub struct AssessCompletionTool;

#[async_trait]
impl Tool for AssessCompletionTool {
    fn name(&self) -> &str {
        "assess_completion"
    }

    fn description(&self) -> &str {
        "Propose requirement-to-evidence coverage for the runtime completion gate."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type":"object", "additionalProperties":false,
            "properties":{
                "summary":{"type":"string"},
                "coverage":{"type":"array","items":{
                    "type":"object","additionalProperties":false,
                    "properties":{
                        "requirementId":{"type":"string"},
                        "evidenceIds":{"type":"array","items":{"type":"string"}},
                        "explanation":{"type":"string"}
                    },
                    "required":["requirementId","evidenceIds","explanation"]
                }}
            },
            "required":["summary","coverage"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let proposal: CompletionAssessment =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: "assess_completion",
                problem: e.to_string(),
            })?;
        let mut report = ctx
            .task
            .lock()
            .map_err(|_| ToolError::Failed("task lock poisoned".into()))?
            .clone()
            .ok_or_else(|| ToolError::Failed("no active task".into()))?;
        report.assessment = Some(proposal.clone());
        let root = ctx.project_root.clone();
        let (candidate, feedback) = tokio::task::spawn_blocking(move || {
            let feedback = WorkspaceSnapshot::capture(&root)
                .and_then(|snapshot| assess(&mut report, &snapshot));
            (report, feedback)
        })
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?;
        let mut guard = ctx
            .task
            .lock()
            .map_err(|_| ToolError::Failed("task lock poisoned".into()))?;
        let report = guard
            .as_mut()
            .filter(|report| report.task_id == candidate.task_id)
            .ok_or_else(|| ToolError::Failed("active task changed during assessment".into()))?;
        report.assessment = Some(proposal);
        report.status = TaskStatus::NeedsVerification;
        let eligible = feedback.is_ok() && candidate.status == TaskStatus::Complete;
        let error = feedback.err().map(|error| error.to_string());
        let result = json!({
            "recorded":true, "eligible":eligible, "gateStatus":candidate.status,
            "status":TaskStatus::NeedsVerification, "error":error,
            "blockers":candidate.blockers
        })
        .to_string();
        Ok(ToolOutput::success(
            result,
            "Completion proposal recorded; runtime persistence and gating remain required",
        ))
    }
}
