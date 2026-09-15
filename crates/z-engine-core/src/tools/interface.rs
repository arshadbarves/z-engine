use async_trait::async_trait;

use super::ToolCtx;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input for {tool}: {problem}")]
    InvalidInput { tool: &'static str, problem: String },
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub result: String,
    pub summary: String,
    pub ok: bool,
}

impl ToolOutput {
    pub fn success(result: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            summary: summary.into(),
            ok: true,
        }
    }

    pub fn failure(result: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            summary: summary.into(),
            ok: false,
        }
    }
}

/// Shared execution seam for built-in and external capabilities.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn concurrency_safe(&self) -> bool {
        true
    }
    fn approval_preview(&self, _input: &serde_json::Value, _ctx: &ToolCtx) -> Option<String> {
        None
    }
    async fn run(&self, input: serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError>;
}
