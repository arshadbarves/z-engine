//! GUI -> engine directives for one session.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, JobId, MessageId, RequestId};
use crate::interaction::{PlanDecision, QuestionAnswer};
use crate::permission::{ApprovalDecision, PermissionMode};
use crate::session::{Effort, RewindScope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Attachment {
    /// A pasted or picked image (base64 without a data-URL prefix).
    Image { media_type: String, data: String },
    /// A file whose contents are attached to the message.
    File { path: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Command {
    /// Start a turn (or queue it as steering when a turn is running).
    Submit {
        text: String,
        #[serde(default)]
        attachments: Vec<Attachment>,
    },
    /// Inject a message at the running turn's next round boundary.
    Steer {
        text: String,
    },
    /// Stop the current round; if `text` is set, send it immediately after.
    Interrupt {
        #[serde(default)]
        text: Option<String>,
    },
    /// Cancel the foreground turn (background jobs keep running).
    Cancel,
    ResolveApproval {
        request_id: RequestId,
        decision: ApprovalDecision,
    },
    /// `answers: None` dismisses the question.
    AnswerQuestion {
        request_id: RequestId,
        #[serde(default)]
        answers: Option<Vec<QuestionAnswer>>,
    },
    ResolvePlan {
        request_id: RequestId,
        decision: PlanDecision,
    },
    SetMode {
        mode: PermissionMode,
    },
    SetModel {
        model: String,
    },
    SetEffort {
        #[serde(default)]
        effort: Option<Effort>,
    },
    Compact {
        #[serde(default)]
        instructions: Option<String>,
    },
    Rewind {
        message_id: MessageId,
        scope: RewindScope,
    },
    KillJob {
        job_id: JobId,
    },
    ApplyAgentChanges {
        agent_id: AgentId,
    },
    DiscardAgentChanges {
        agent_id: AgentId,
    },
    /// A slash command, e.g. `name: "review", args: "src/"`.
    RunCommand {
        name: String,
        #[serde(default)]
        args: String,
    },
    /// `!cmd`: run a shell command without the model.
    Shell {
        command: String,
    },
    /// Replace the queued steering messages.
    EditQueue {
        queued: Vec<String>,
    },
    ReloadExtensions,
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_json_deserializes() {
        let cmd: Command = serde_json::from_value(serde_json::json!({
            "type": "resolveApproval",
            "requestId": "req_1",
            "decision": {"type": "allowSession", "rule": "Bash(cargo test:*)"}
        }))
        .unwrap();
        assert!(matches!(cmd, Command::ResolveApproval { .. }));

        let submit: Command =
            serde_json::from_value(serde_json::json!({"type": "submit", "text": "hi"})).unwrap();
        assert_eq!(
            submit,
            Command::Submit {
                text: "hi".into(),
                attachments: vec![]
            }
        );
    }
}
