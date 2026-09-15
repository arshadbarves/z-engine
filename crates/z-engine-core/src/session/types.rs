use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One persisted transcript event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Meta {
        model: String,
        project_root: String,
    },
    UserMsg {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
    },
    AssistantMsg {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<PersistedToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    Note {
        text: String,
    },
    Title {
        text: String,
    },
    /// Turn finished, independently of verified task completion.
    TurnEnd {
        outcome: String,
    },
    /// Versioned verification evidence; never a provider chat message.
    TaskUpdated {
        report: Box<crate::verification::TaskReport>,
    },
    Ack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub path: PathBuf,
    pub ulid: String,
    /// Persisted title, or the first user message as a fallback.
    pub first_user_msg: Option<String>,
    pub modified: std::time::SystemTime,
    pub project_root: Option<String>,
    /// Latest unacknowledged turn/task outcome.
    pub unread_outcome: Option<String>,
}

#[derive(Debug)]
pub struct Replayed {
    pub working: Vec<z_engine_provider::ChatMessage>,
    /// Note texts and recorded `update_context_notes` arguments.
    pub notes_replayed: Vec<String>,
}
