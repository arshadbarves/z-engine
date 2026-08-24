//! Typed messages crossing the core↔UI boundary.
//!
//! Core→TUI [`Event`]s arrive over an unbounded channel; TUI→core
//! [`Command`]s flow the other way (spec §3). These types are the *only*
//! coupling between the two worlds.

/// Scope of an approval answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Once,
    AlwaysSession { rule: String },
    AlwaysPersist { rule: String },
}

/// UI → core directives.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// User submitted a new task message.
    SubmitMessage(String),
    /// Approval modal answered "yes" with a scope (see ApprovalDecision).
    Approve { id: u64, decision: ApprovalDecision },
    /// Approval modal answered "no".
    Deny { id: u64 },
    /// Esc / interrupt: abort the current turn instantly, mid-stream OK.
    Abort,
    /// Slash-command `/compact`: force context compaction now.
    Compact,
    /// Slash-command `/notes`: dump the L1 notes block as a status note.
    RequestNotes,
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
        /// Rich preview (e.g. unified diff for write/edit).
        detail_preview: Option<String>,
        /// False when the target lies outside the project root — "persist"
        /// is disabled there (spec section 5).
        can_persist: bool,
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
