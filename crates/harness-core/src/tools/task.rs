//! `task` — spawns an isolated sub-agent loop for exploration/research
//! (spec §9 v0.7). Only the final summary returns to the parent
//! transcript, keeping parent context small while the sub-agent burns its
//! own tokens on broad reading.

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile};

const DEFAULT_MAX_ROUNDS: u32 = 8;
const MAX_ROUNDS_CAP: u32 = 24;

#[derive(Debug)]
pub struct TaskTool;

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Delegate a self-contained research or exploration question to an \
         isolated read-only sub-agent. It can read files, glob and grep the \
         project, but cannot edit anything. Only its final answer enters \
         this conversation — ideal for broad lookups whose intermediate \
         output would waste context."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The complete question or research task for the sub-agent. Be specific about what to report back."
                },
                "max_tool_rounds": {
                    "type": "integer",
                    "description": "Cap on the sub-agent's tool rounds (default 8, max 24)."
                }
            },
            "required": ["prompt"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        true // fully isolated state
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "task",
            problem: "input must be an object".into(),
        })?;
        let prompt = obj
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "task",
                problem: "`prompt` must be a non-empty string".into(),
            })?;
        let max_rounds = obj
            .get("max_tool_rounds")
            .and_then(Value::as_u64)
            .map(|v| v as u32)
            .unwrap_or(DEFAULT_MAX_ROUNDS)
            .clamp(1, MAX_ROUNDS_CAP);

        if ctx.aborted() {
            return Err(ToolError::Failed("aborted".into()));
        }

        let runner = ctx
            .task_runner
            .as_ref()
            .ok_or_else(|| ToolError::Failed("sub-agents are not available in this mode".into()))?;

        tracing::info!(prompt_len = prompt.len(), max_rounds, "spawning sub-agent");
        match runner(prompt.to_string(), max_rounds).await {
            Ok(summary) => {
                let body = format!("sub-agent report:\n{summary}");
                let result = truncate_with_tempfile(&body, ctx);
                Ok(ToolOutput::success(
                    result,
                    format!("task: {}", summary.chars().take(60).collect::<String>()),
                ))
            }
            Err(e) => {
                // Sub-agent failures are model-visible data (self-correction).
                Ok(ToolOutput::failure(
                    format!("ERROR: {e}"),
                    format!("task failed: {e}"),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn ctx_with_runner(runner: Option<super::super::SubAgentRunner>) -> ToolCtx {
        let mut ctx = ToolCtx::new(
            std::path::PathBuf::from("."),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        );
        ctx.task_runner = runner;
        ctx
    }

    #[tokio::test]
    async fn returns_summary_as_result() {
        let runner: super::super::SubAgentRunner =
            Arc::new(|_p, _m| Box::pin(async { Ok("the answer is 42".to_string()) }));
        let out = TaskTool
            .run(
                json!({"prompt": "research something"}),
                &ctx_with_runner(Some(runner)),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("the answer is 42"));
    }

    #[tokio::test]
    async fn missing_runner_is_model_visible_error() {
        let err = TaskTool
            .run(json!({"prompt": "x"}), &ctx_with_runner(None))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not available"));
    }

    #[tokio::test]
    async fn sub_agent_failure_becomes_error_result() {
        let runner: super::super::SubAgentRunner =
            Arc::new(|_p, _m| Box::pin(async { Err("exploded".to_string()) }));
        let out = TaskTool
            .run(json!({"prompt": "x"}), &ctx_with_runner(Some(runner)))
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.result.contains("exploded"));
    }
}
