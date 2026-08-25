//! Typed messages crossing the core↔UI boundary.
//!
//! Core→TUI [`Event`]s arrive over an unbounded channel; TUI→core
//! [`Command`]s flow the other way (spec §3). These types are the *only*
//! coupling between the two worlds.

/// Interaction permission mode (Claude Code parity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Prompt for every gated call.
    Normal,
    /// Auto-approve file edits; bash still prompts.
    AutoAcceptEdits,
    /// Deny all mutating calls; reads only.
    Plan,
}

impl PermissionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::AutoAcceptEdits => "auto-accept edits",
            Self::Plan => "plan",
        }
    }
}

/// Scope of an approval answer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// Shift+Tab: cycle/set the permission mode.
    SetMode(PermissionMode),
    /// `/model <id>`: hot-switch the provider model.
    SetModel(String),
    /// `!<cmd>` shell passthrough executed locally (no model involvement).
    Shell(String),
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
    ReasoningDelta(String),
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
        /// Parsed command for bash calls (drives rule suggestions).
        bash_command: Option<String>,
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

/// JSON contract shared with the GUI frontend (camelCase, tagged by `type`).
impl serde::Serialize for Event {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde_json::json;
        let v = match self {
            Event::TurnStarted => json!({"type": "turnStarted"}),
            Event::TokenDelta(t) => json!({"type": "tokenDelta", "text": t}),
            Event::ReasoningDelta(r) => json!({"type": "reasoningDelta", "text": r}),
            Event::ToolCallStarted { name, preview } => json!({
                "type": "toolCallStarted", "name": name, "preview": preview
            }),
            Event::ToolCallFinished {
                name,
                ok,
                duration_ms,
                summary,
            } => json!({
                "type": "toolCallFinished", "name": name, "ok": ok,
                "durationMs": duration_ms, "summary": summary
            }),
            Event::ApprovalRequired {
                id,
                tool,
                input_preview,
                suggested_rule,
                detail_preview,
                can_persist,
                bash_command,
            } => json!({
                "type": "approvalRequired", "id": id, "tool": tool,
                "inputPreview": input_preview, "suggestedRule": suggested_rule,
                "detailPreview": detail_preview, "canPersist": can_persist,
                "bashCommand": bash_command
            }),
            Event::UsageUpdated {
                prompt_tokens,
                completion_tokens,
            } => json!({
                "type": "usageUpdated",
                "promptTokens": prompt_tokens, "completionTokens": completion_tokens
            }),
            Event::StatusNote(s) => json!({"type": "statusNote", "text": s}),
            Event::TurnCompleted {
                prompt_tokens,
                completion_tokens,
            } => json!({
                "type": "turnCompleted",
                "promptTokens": prompt_tokens, "completionTokens": completion_tokens
            }),
            Event::TurnAborted => json!({"type": "turnAborted"}),
            Event::Error(m) => json!({"type": "error", "message": m}),
        };
        v.serialize(serializer)
    }
}
