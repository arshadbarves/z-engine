//! `ExitPlanMode`: submits the plan for review in plan mode.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::{PermissionMode, PlanDecision};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::Fields;
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        names::EXIT_PLAN_MODE
    }

    fn description(&self) -> String {
        prompts::EXIT_PLAN_MODE.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "plan": {"type": "string", "description": "The complete plan in markdown."}
            }),
            &["plan"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, _input: &Value, _ctx: &ToolCtx) -> String {
        "Propose plan".to_string()
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let plan = Fields::new(&input)?.non_empty("plan")?;
        if ctx.mode != PermissionMode::Plan {
            return Err(ToolError::unavailable(format!(
                "ExitPlanMode is only available in plan mode, and the current mode is {}. Continue with the task directly.",
                ctx.mode.label()
            )));
        }
        let interaction = ctx.ports.interaction()?;
        if !interaction.can_ask(ctx) {
            return Err(ToolError::unavailable(
                "ExitPlanMode is not available to subagents; put the plan in your final report instead.",
            ));
        }
        let decision = ctx
            .until_cancelled(interaction.propose_plan(ctx, plan.to_string()))
            .await?
            .map_err(ToolError::failed)?;
        Ok(match decision {
            PlanDecision::Approve { mode, edited_plan } => {
                let mut text = format!(
                    "The user approved the plan. The permission mode is now {}, so you can start implementing it.",
                    mode.label()
                );
                if let Some(edited) = edited_plan.filter(|edited| !edited.trim().is_empty()) {
                    text.push_str(&format!(
                        "\n\nThe user edited the plan; implement this version:\n\n{edited}"
                    ));
                }
                ToolOutput::text(text, format!("Plan approved ({})", mode.label()))
            }
            PlanDecision::Revise { feedback } => ToolOutput::text(
                format!(
                    "The user wants changes before approving the plan. Plan mode is still active: revise the plan and submit it again with ExitPlanMode.\n\nFeedback:\n{feedback}"
                ),
                "Changes requested",
            ),
        })
    }
}
