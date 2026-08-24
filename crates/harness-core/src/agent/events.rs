//! Typed messages crossing the core↔UI boundary.
//!
//! Core→TUI [`Event`]s arrive over an unbounded channel; TUI→core
//! [`Command`]s flow the other way (spec §3). These types are the *only*
//! coupling between the two worlds.

/// UI → core directives.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// User submitted a new task message.
    SubmitMessage(String),
    /// Approval modal answered "yes" — optionally persisting a bash-prefix
    /// rule for the rest of the session ("always this prefix").
    Approve {
        id: u64,
        prefix_rule: Option<String>,
    },
    /// Approval modal answered "no".
    Deny { id: u64 },
    /// Esc / interrupt: abort the current turn instantly, mid-stream OK.
    Abort,
    /// Graceful shutdown of the agent task.
    Shutdown,
}

/// Core → UI notifications, in rough lifecycle order.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    TurnStarted,
    TokenDelta(String),
    ToolCallStarted {
        name: String,
        preview: String,
    },
    ToolCallFinished {
        name: String,
        ok: bool,
        duration_ms: u64,
        summary: String,
    },
    /// A gated action awaits an answer from the approval modal.
    ApprovalRequired {
        id: u64,
        tool: String,
        input_preview: String,
        suggested_rule: Option<String>,
    },
    UsageUpdated {
        prompt_tokens: u64,
        completion_tokens: u64,
    },
    StatusNote(String),
    TurnCompleted {
        prompt_tokens: u64,
        completion_tokens: u64,
    },
    TurnAborted,
    Error(String),
}
