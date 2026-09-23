//! The authoritative in-memory state of one session, rebuilt from the log
//! on open and after conversation rewinds. Only short critical sections
//! touch it; nothing here performs I/O.

use std::collections::BTreeMap;

use z_engine_protocol::{
    AgentId, AgentInfo, CheckRecord, CheckpointInfo, CompactionMarker, Effort, Message,
    PermissionMode, TodoItem, TurnOutcome, TurnRecord, Usage,
};
use z_engine_store::ReplayState;

use crate::run::ContextMeter;
use crate::verify::Mutation;

#[derive(Debug, Clone)]
pub(crate) struct SessionState {
    pub created_at: u64,
    pub updated_at: u64,
    pub legacy: bool,
    /// Every visible main-agent message, compacted history included.
    pub transcript: Vec<Message>,
    /// What the main agent's model sees.
    pub working: Vec<Message>,
    pub compactions: Vec<CompactionMarker>,
    pub turns: Vec<TurnRecord>,
    pub todos: BTreeMap<AgentId, Vec<TodoItem>>,
    pub agents: Vec<AgentInfo>,
    pub checks: Vec<CheckRecord>,
    /// Checkpoints with their shadow commit.
    pub checkpoints: Vec<(CheckpointInfo, String)>,
    pub mode: PermissionMode,
    pub model: String,
    pub effort: Option<Effort>,
    pub title: Option<String>,
    /// Session totals, side requests included.
    pub usage: Usage,
    pub cost_usd: f64,
    /// Cumulative usage per agent.
    pub agent_usage: BTreeMap<AgentId, Usage>,
    /// Steering messages waiting for the next round boundary.
    pub queue: Vec<String>,
    pub context_tokens: u64,
    pub context_limit: u64,
    /// Context occupancy of the main agent's working set.
    pub meter: ContextMeter,
    /// The previous turn was cancelled or interrupted.
    pub interrupted: bool,
    /// A `!cmd` ran since the last turn started.
    pub external_mutation: bool,
    /// What the current (or last) main-agent turn changed.
    pub mutation: Mutation,
}

impl SessionState {
    /// `model`/`effort` come from the log when it recorded them, else from
    /// the current settings.
    pub(crate) fn from_replay(
        replay: ReplayState,
        legacy: bool,
        updated_at: u64,
        default_model: &str,
        default_effort: Option<Effort>,
    ) -> Self {
        let created_at = replay.info.as_ref().map_or(updated_at, |info| info.2);
        let interrupted = replay.turns.last().is_some_and(|turn| {
            matches!(
                turn.outcome,
                TurnOutcome::Cancelled | TurnOutcome::Interrupted
            )
        });
        let mut agent_usage = BTreeMap::new();
        agent_usage.insert(AgentId::main(), replay.usage);
        Self {
            created_at,
            updated_at: updated_at.max(created_at),
            legacy,
            transcript: replay.transcript,
            working: replay.working,
            compactions: replay.compactions,
            turns: replay.turns,
            todos: replay.todos,
            agents: replay.agents,
            checks: replay.checks,
            checkpoints: replay.checkpoints,
            mode: replay.mode,
            model: replay
                .model
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| default_model.to_string()),
            effort: replay.effort.or(default_effort),
            title: replay.title,
            usage: replay.usage,
            cost_usd: replay.cost_usd,
            agent_usage,
            queue: Vec::new(),
            context_tokens: 0,
            context_limit: 0,
            meter: ContextMeter::default(),
            interrupted,
            external_mutation: false,
            mutation: Mutation::default(),
        }
    }

    pub(crate) fn todos_of(&self, agent: &AgentId) -> &[TodoItem] {
        self.todos.get(agent).map_or(&[], Vec::as_slice)
    }
}
