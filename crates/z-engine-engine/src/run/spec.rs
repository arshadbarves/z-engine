//! What parameterizes one agent run, so the same loop serves the main
//! agent and (later) subagents: identity, prompt, tools, model, mode,
//! budget, root directory, depth, working resources and cancellation.

use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use z_engine_protocol::{AgentId, PermissionMode, TurnOutcome, Usage, VerificationOutcome};

use crate::session::{AgentResources, SessionCore};

/// Which registry tools an agent is offered.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToolFilter {
    /// `None` offers every tool.
    pub allow: Option<Vec<String>>,
    pub deny: Vec<String>,
}

impl ToolFilter {
    pub(crate) fn permits(&self, name: &str) -> bool {
        let allowed = self
            .allow
            .as_ref()
            .is_none_or(|allow| allow.iter().any(|tool| tool == name));
        allowed && !self.deny.iter().any(|tool| tool == name)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ModelChoice {
    /// Follows the session's main model (`SetModel` applies next round).
    Session,
    #[expect(
        dead_code,
        reason = "subagent definitions pick their own model (phase 6)"
    )]
    Fixed(String),
}

#[derive(Debug, Clone)]
pub(crate) struct AgentSpec {
    pub agent_id: AgentId,
    pub base_prompt: String,
    pub tools: ToolFilter,
    pub model: ModelChoice,
    /// `None` follows the session's permission mode.
    pub mode: Option<PermissionMode>,
    pub max_turns: u32,
    /// Project root, or the agent's worktree.
    pub root: PathBuf,
    #[expect(dead_code, reason = "the subagent depth limit reads it (phase 6)")]
    pub depth: u32,
}

impl AgentSpec {
    pub(crate) fn main(root: PathBuf, max_turns: u32) -> Self {
        Self {
            agent_id: AgentId::main(),
            base_prompt: z_engine_prompts::system::MAIN.to_string(),
            tools: ToolFilter::default(),
            model: ModelChoice::Session,
            mode: None,
            max_turns: max_turns.max(1),
            root,
            depth: 0,
        }
    }

    pub(crate) fn is_main(&self) -> bool {
        self.agent_id.is_main()
    }
}

/// Everything a run and its tool batches share.
#[derive(Debug, Clone)]
pub(crate) struct RunContext {
    pub core: Arc<SessionCore>,
    pub spec: AgentSpec,
    pub resources: AgentResources,
    pub cancel: CancellationToken,
}

impl RunContext {
    pub(crate) fn mode(&self) -> PermissionMode {
        self.spec.mode.unwrap_or_else(|| self.core.mode())
    }

    pub(crate) fn model(&self) -> String {
        match &self.spec.model {
            ModelChoice::Session => self.core.main_model(),
            ModelChoice::Fixed(model) => model.clone(),
        }
    }
}

/// How a run ended and what it cost.
#[derive(Debug, Clone)]
pub(crate) struct RunOutcome {
    pub outcome: TurnOutcome,
    pub usage: Usage,
    pub cost_usd: f64,
    /// Files changed during the run.
    pub mutated: bool,
    /// The badge from the stop boundary, when the run reached it.
    pub verification: Option<VerificationOutcome>,
    /// Text of the last assistant message.
    #[expect(dead_code, reason = "a subagent's report to its parent (phase 6)")]
    pub final_text: String,
}
