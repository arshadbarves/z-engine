//! `[permissions]`: the permission mode and the policy engine's rule lists.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use z_engine_protocol::PermissionMode;

use super::lenient;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct PermissionSettings {
    #[serde(deserialize_with = "lenient::permission_mode")]
    pub mode: PermissionMode,
    /// Rules such as `Read`, `Bash(cargo test:*)`, or `Edit(src/**)`.
    /// Rule lists union across layers.
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    /// Directories outside the project the agent may work in.
    pub additional_directories: Vec<String>,
    /// Run recognised read-only shell commands without asking.
    pub auto_allow_read_only_bash: bool,
}

impl Default for PermissionSettings {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            additional_directories: Vec::new(),
            auto_allow_read_only_bash: true,
        }
    }
}

/// Which permission list a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum RuleKind {
    Allow,
    Ask,
    Deny,
}

impl RuleKind {
    /// The key of this list inside `[permissions]`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}
