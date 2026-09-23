//! Engine -> GUI notifications. Every event is wrapped in an
//! [`EventEnvelope`] carrying the session id and a per-session sequence.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::agents::AgentInfo;
use crate::content::Message;
use crate::ids::{AgentId, CallId, MessageId, RequestId, SessionId, TurnId};
use crate::interaction::{Question, TodoItem};
use crate::jobs::JobInfo;
use crate::permission::{ApprovalRequest, PermissionMode};
use crate::session::{
    CheckpointInfo, CompactionMarker, ContextBreakdown, Effort, SessionSnapshot, SessionStatus,
    TurnRecord,
};
use crate::usage::Usage;
use crate::verification::{CheckRecord, VerificationOutcome};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EventEnvelope {
    pub session_id: SessionId,
    #[ts(type = "number")]
    pub seq: u64,
    pub event: Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolStatus {
    Ok,
    Error,
    Denied,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Event {
    /// Full state; sent on open and after rewinds or reloads.
    Snapshot {
        snapshot: Box<SessionSnapshot>,
    },
    StatusChanged {
        status: SessionStatus,
    },
    /// A persisted user message (a new turn or an injected steering note).
    UserMessage {
        message: Message,
        turn_id: Option<TurnId>,
        steering: bool,
    },
    TurnStarted {
        turn_id: TurnId,
        message_id: MessageId,
    },
    TurnFinished {
        turn: TurnRecord,
    },
    AssistantStarted {
        agent_id: AgentId,
        message_id: MessageId,
    },
    TextDelta {
        agent_id: AgentId,
        message_id: MessageId,
        text: String,
    },
    ThinkingDelta {
        agent_id: AgentId,
        message_id: MessageId,
        text: String,
    },
    /// The authoritative assembled assistant message.
    AssistantFinished {
        agent_id: AgentId,
        message: Message,
    },
    ToolStarted {
        agent_id: AgentId,
        call_id: CallId,
        tool: String,
        title: String,
        input: Value,
    },
    ToolProgress {
        agent_id: AgentId,
        call_id: CallId,
        text: String,
    },
    ToolFinished {
        agent_id: AgentId,
        call_id: CallId,
        status: ToolStatus,
        summary: String,
        /// Truncated result text for the tool card.
        output: String,
        #[ts(type = "number")]
        duration_ms: u64,
    },
    ApprovalRequested {
        request: ApprovalRequest,
    },
    ApprovalResolved {
        request_id: RequestId,
        allowed: bool,
    },
    QuestionAsked {
        request_id: RequestId,
        agent_id: AgentId,
        questions: Vec<Question>,
    },
    QuestionResolved {
        request_id: RequestId,
        answered: bool,
    },
    PlanProposed {
        request_id: RequestId,
        agent_id: AgentId,
        plan: String,
    },
    PlanResolved {
        request_id: RequestId,
        approved: bool,
    },
    TodosUpdated {
        agent_id: AgentId,
        todos: Vec<TodoItem>,
    },
    AgentStarted {
        info: AgentInfo,
    },
    AgentUpdated {
        info: AgentInfo,
    },
    JobUpdated {
        job: JobInfo,
    },
    UsageUpdated {
        agent_id: AgentId,
        /// Cumulative usage of this agent.
        usage: Usage,
        session_usage: Usage,
        cost_usd: f64,
        #[ts(type = "number")]
        context_tokens: u64,
        #[ts(type = "number")]
        context_limit: u64,
    },
    ModeChanged {
        mode: PermissionMode,
    },
    ModelChanged {
        model: String,
    },
    EffortChanged {
        effort: Option<Effort>,
    },
    Compacted {
        marker: CompactionMarker,
    },
    CheckpointCreated {
        checkpoint: CheckpointInfo,
    },
    CheckRecorded {
        record: CheckRecord,
    },
    VerificationChanged {
        outcome: VerificationOutcome,
    },
    HookRan {
        hook_event: String,
        command: String,
        blocked: bool,
        message: Option<String>,
    },
    /// Markdown output of an informational slash command.
    CommandOutput {
        name: String,
        markdown: String,
    },
    ContextReport {
        breakdown: ContextBreakdown,
    },
    Notice {
        level: NoticeLevel,
        text: String,
    },
    /// The provider asked us to back off; a retry is scheduled.
    Retrying {
        attempt: u32,
        #[ts(type = "number")]
        delay_ms: u64,
        reason: String,
    },
    TitleChanged {
        title: String,
    },
    /// Steering messages waiting for the next round boundary.
    QueueChanged {
        queued: Vec<String>,
    },
    /// The project defines hooks, MCP servers or checks that stay off
    /// until the user trusts it (`defines` names them, e.g. `hooks`).
    TrustRequired {
        project_root: String,
        defines: Vec<String>,
    },
    Error {
        message: String,
    },
}

impl Event {
    pub fn notice(level: NoticeLevel, text: impl Into<String>) -> Self {
        Self::Notice {
            level,
            text: text.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_use_camel_case_tags_and_fields() {
        let event = Event::TextDelta {
            agent_id: AgentId::main(),
            message_id: MessageId::from("m1"),
            text: "hi".into(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "textDelta");
        assert_eq!(json["agentId"], "main");
        assert_eq!(json["messageId"], "m1");
        let back: Event = serde_json::from_value(json).unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn trust_required_round_trips() {
        let event = Event::TrustRequired {
            project_root: "/work/app".into(),
            defines: vec!["hooks".into(), "MCP servers".into()],
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "trustRequired");
        assert_eq!(json["projectRoot"], "/work/app");
        assert_eq!(json["defines"][1], "MCP servers");
        let back: Event = serde_json::from_value(json).unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn envelope_nests_the_event() {
        let envelope = EventEnvelope {
            session_id: SessionId::from("S"),
            seq: 3,
            event: Event::notice(NoticeLevel::Warn, "careful"),
        };
        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["sessionId"], "S");
        assert_eq!(json["event"]["type"], "notice");
        assert_eq!(json["event"]["level"], "warn");
    }
}
