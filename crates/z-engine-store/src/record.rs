//! One line of a v2 log. Records are tagged by `kind` (camelCase) and are
//! decoded line by line, so an unknown kind costs one skipped line.
//!
//! Writers append a user `Message` before the `TurnStarted` that references
//! it, so a conversation rewind to that message also drops its turn.

use serde::{Deserialize, Serialize};
use z_engine_protocol::{
    AgentId, AgentInfo, CheckRecord, CheckpointInfo, CompactionMarker, Effort, Message, MessageId,
    PermissionMode, Question, QuestionAnswer, RequestId, SessionId, TodoItem, TurnId, TurnRecord,
    Usage, decisions::TaskViewInfo,
};

/// Schema of `log.jsonl` and `meta.json` written by this crate.
pub const SESSION_SCHEMA: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LogRecord {
    /// First record of every session log.
    SessionStarted {
        schema: u32,
        session_id: SessionId,
        project_root: String,
        model: String,
        mode: PermissionMode,
        created_at: u64,
    },
    /// A main-agent transcript message: user input, assistant output, or a
    /// user message of tool results.
    Message {
        message: Message,
        turn_id: Option<TurnId>,
    },
    TurnStarted {
        turn_id: TurnId,
        message_id: MessageId,
        started_at: u64,
    },
    TurnFinished {
        turn: TurnRecord,
    },
    /// The complete todo list of one agent; replaces the previous one.
    Todos {
        agent_id: AgentId,
        todos: Vec<TodoItem>,
    },
    PlanProposed {
        request_id: RequestId,
        agent_id: AgentId,
        plan: String,
    },
    PlanResolved {
        request_id: RequestId,
        approved: bool,
        feedback: Option<String>,
        final_plan: Option<String>,
    },
    QuestionAsked {
        request_id: RequestId,
        agent_id: AgentId,
        questions: Vec<Question>,
    },
    /// `answers` is `None` when the user dismissed the questions.
    QuestionAnswered {
        request_id: RequestId,
        answers: Option<Vec<QuestionAnswer>>,
    },
    /// Audit trail of an approval decision; replay does not act on it.
    Approval {
        request_id: RequestId,
        tool: String,
        title: String,
        allowed: bool,
        rule: Option<String>,
    },
    /// Latest state of one agent run.
    AgentUpdated {
        info: AgentInfo,
    },
    Check {
        record: CheckRecord,
    },
    /// `snapshot` is the shadow commit holding the code state.
    Checkpoint {
        info: CheckpointInfo,
        snapshot: String,
    },
    /// `summary` (a user-role message) replaces the history before
    /// `marker.keep_from` in the working set; the transcript keeps it all.
    Compacted {
        marker: CompactionMarker,
        summary: Message,
    },
    /// With `conversation`, replay drops the message and everything recorded
    /// after it up to this record. `code` alone changes nothing on replay.
    Rewound {
        message_id: MessageId,
        conversation: bool,
        code: bool,
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
    Title {
        title: String,
    },
    /// Usage of side requests (titles, summaries) not covered by a
    /// `TurnFinished`.
    Usage {
        agent_id: AgentId,
        usage: Usage,
        cost_usd: f64,
    },
    /// Imported v1 notes and other annotations; replay ignores it.
    Note {
        text: String,
    },
    /// `decisions_task_view`: the working set became the messages with the
    /// `working` ids, in order (`index` is the index message, which is not
    /// in the transcript); with `view.restored`, the full history again.
    TaskView {
        view: TaskViewInfo,
        #[serde(default)]
        working: Vec<MessageId>,
        #[serde(default)]
        index: Option<Message>,
    },
}
