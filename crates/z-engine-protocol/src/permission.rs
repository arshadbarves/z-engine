//! Permission modes and the approval exchange.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::ids::{AgentId, CallId, RequestId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PermissionMode {
    /// Ask before gated actions.
    #[default]
    Default,
    /// Auto-approve file edits inside allowed directories.
    AcceptEdits,
    /// Read-only exploration; `ExitPlanMode` proposes a plan.
    Plan,
    /// Approve everything except explicit deny rules.
    Bypass,
}

impl PermissionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Plan => "plan",
            Self::Bypass => "bypass",
        }
    }

    /// Accepts v2 names plus v1 and Claude Code spellings.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "default" | "normal" | "ask" => Some(Self::Default),
            "acceptEdits" | "accept_edits" | "accept-edits" | "auto-accept edits" => {
                Some(Self::AcceptEdits)
            }
            "plan" | "readOnly" | "read-only" => Some(Self::Plan),
            "bypass" | "bypassPermissions" => Some(Self::Bypass),
            _ => None,
        }
    }
}

/// The user's answer to an approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ApprovalDecision {
    AllowOnce,
    /// Allow and add `rule` for the rest of the session.
    AllowSession {
        rule: String,
    },
    /// Allow and persist `rule` to the project's local settings.
    AllowProject {
        rule: String,
    },
    /// Refuse; optional feedback is returned to the model.
    Deny {
        #[serde(default)]
        feedback: Option<String>,
    },
}

impl ApprovalDecision {
    pub fn allows(&self) -> bool {
        !matches!(self, Self::Deny { .. })
    }
}

/// Rich detail rendered by the approval card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Preview {
    Diff {
        path: String,
        diff: String,
    },
    Command {
        command: String,
        #[serde(default)]
        description: Option<String>,
    },
    Text {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalRequest {
    pub request_id: RequestId,
    pub agent_id: AgentId,
    pub call_id: CallId,
    pub tool: String,
    /// One-line description, e.g. "Edit src/main.rs".
    pub title: String,
    pub input: Value,
    pub preview: Option<Preview>,
    /// Why the policy asked (matched rule, outside directory, ...).
    pub reason: String,
    pub suggested_rule: Option<String>,
    /// False for targets outside allowed directories.
    pub can_persist: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parse_accepts_legacy_spellings() {
        assert_eq!(
            PermissionMode::parse("normal"),
            Some(PermissionMode::Default)
        );
        assert_eq!(
            PermissionMode::parse("bypassPermissions"),
            Some(PermissionMode::Bypass)
        );
        assert_eq!(PermissionMode::parse("nope"), None);
    }

    #[test]
    fn decisions_are_tagged() {
        let json = serde_json::to_value(ApprovalDecision::Deny {
            feedback: Some("use cargo".into()),
        })
        .unwrap();
        assert_eq!(json["type"], "deny");
        assert_eq!(json["feedback"], "use cargo");
        let back: ApprovalDecision =
            serde_json::from_value(serde_json::json!({"type": "allowOnce"})).unwrap();
        assert!(back.allows());
    }
}
