//! Startup configuration for the agent loop (built once at startup).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::context::compact;

/// Everything the loop needs; built once at startup (headless or TUI).
#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub model: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub project_root: PathBuf,
    pub tmp_dir: PathBuf,
    /// Seed bash-prefix allow rules (from config files).
    pub initial_allow_rules: Vec<String>,
    /// Context budget (spec §6); drives warnings + auto-compaction.
    pub max_context_tokens: u32,
    /// Explicit per-request output ceiling (max_tokens).
    pub max_output_tokens: u32,
    /// Lifecycle shell hooks (`session_start`, `turn_completed`).
    pub hooks: BTreeMap<String, String>,
    /// Auto-compaction trigger point as a percent of the budget.
    pub compact_at_percent: u8,
    /// Verbatim L2 tail size for compaction.
    pub keep_recent_messages: usize,
    /// Run the post-edit reviewer pass (spec section 9 v0.9).
    pub review_enabled: bool,
    /// External MCP stdio servers to register at startup (v0.9).
    pub mcp_servers: Vec<crate::mcp::McpServerConfig>,
    /// Tools auto-allowed without gating (e.g. trusted MCP externals).
    pub auto_allow_tools: Vec<String>,
    /// Starting permission mode (spec section 9 v1.1 parity).
    pub initial_mode: crate::agent::events::PermissionMode,
    /// Windows shell override (e.g., "C:\Program Files\Git\bin\bash.exe").
    pub shell_path: Option<String>,
}

impl LoopConfig {
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: base_url.into(),
            api_key: None,
            project_root: PathBuf::from("."),
            tmp_dir: std::env::temp_dir(),
            initial_allow_rules: Vec::new(),
            max_context_tokens: 120_000,
            max_output_tokens: 16_384,
            hooks: BTreeMap::new(),
            compact_at_percent: 92,
            keep_recent_messages: compact::DEFAULT_KEEP_RECENT,
            review_enabled: true,
            mcp_servers: Vec::new(),
            auto_allow_tools: Vec::new(),
            initial_mode: crate::agent::events::PermissionMode::Normal,
            shell_path: None,
        }
    }
}
