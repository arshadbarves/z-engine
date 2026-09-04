//! Tool subsystem: the [`Tool`] trait (the only extension seam), the
//! execution context handed to every tool, the registry advertised to the
//! model, and shared output-truncation plumbing.
//!
//! Loop contract reminders (spec §4.2):
//! - tool errors are *data*: they become tool-result messages so the model
//!   can self-correct — they never crash the loop;
//! - oversized outputs are truncated head+tail for the transcript while the
//!   full text lands in a temp file whose path is embedded in the result.

pub mod bash;
pub mod checkpoint;
mod checkpoint_session;
pub mod context_notes;
pub mod edit_file;
pub mod file_state;
pub mod glob;
pub mod grep;
pub mod lsp_tools;
pub mod read_file;
pub mod task;
pub mod write_file;

mod bash_script;
mod checkpoint_restore;
mod context;
mod edit_ladder;
mod fsutil;
mod grep_backend;
mod proc_helpers;
mod shell;
#[cfg(windows)]
mod shell_detect;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

pub use checkpoint::CheckpointStore;
pub use checkpoint_session::{SessionFileChange, list_session_changes, session_diff_for};
pub use context::{SubAgentFuture, SubAgentRunner, ToolCtx, ToolOutputChunk};
pub(crate) use fsutil::atomic_write;
pub use fsutil::{MAX_TOOL_OUTPUT_CHARS, truncate_with_tempfile, unified_diff};

/// Initialize the shell resolver with an optional config override.
/// Must be called once at startup before any shell operations.
pub fn init_shell(config_shell_path: Option<&str>) {
    shell::init(config_shell_path);
}

/// Spawn `sh -c` (or the resolved Windows shell) for a one-shot command line.
pub(crate) fn shell_line(command: &str) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(shell::program_path());
    if shell::is_powershell() {
        c.arg(shell::flag())
            .arg(shell::powershell_command_flag())
            .arg(command);
    } else {
        c.arg(shell::flag()).arg(command);
    }
    // Never pop a visible console window for hooks on Windows.
    #[cfg(windows)]
    c.creation_flags(shell::CREATE_NO_WINDOW);
    c
}

/// Errors from tool execution. Display strings go to the model verbatim.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input for {tool}: {problem}")]
    InvalidInput { tool: &'static str, problem: String },
    #[error("{0}")]
    Failed(String),
}

/// Result payload of a successful or failed run.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Exact text that enters the transcript as the tool-result content.
    pub result: String,
    /// One-line human summary for TUI events.
    pub summary: String,
    pub ok: bool,
}

impl ToolOutput {
    pub fn success(result: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            summary: summary.into(),
            ok: true,
        }
    }

    pub fn failure(result: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            summary: summary.into(),
            ok: false,
        }
    }
}

/// The single extension seam (spec §4.1). Built-ins and future MCP tools
/// implement exactly this; the agent loop never knows the difference.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema object describing the input.
    fn parameters_schema(&self) -> serde_json::Value;
    /// Whether parallel execution alongside other safe tools is OK.
    /// Mutating/stateful tools (bash) return false.
    fn concurrency_safe(&self) -> bool {
        true
    }
    /// Rich human/model-facing preview shown in the approval modal for gated
    /// calls (e.g. a unified diff). None falls back to raw input JSON.
    fn approval_preview(&self, _input: &serde_json::Value, _ctx: &ToolCtx) -> Option<String> {
        None
    }
    async fn run(&self, input: serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError>;
}

/// Registry of available tools; produces the provider-facing definitions.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if !self.tools.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn names(&self) -> &[String] {
        &self.order
    }

    /// Read-only subset for sub-agents (spec section 9 v0.7): isolated
    /// explore/research loops cannot mutate the workspace.
    pub fn readonly_subset() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(read_file::ReadFileTool));
        reg.register(Arc::new(glob::GlobTool));
        reg.register(Arc::new(grep::GrepTool));
        reg
    }

    /// Definitions advertised in chat-completion requests.
    pub fn defs(&self) -> Vec<z_engine_provider::ToolDef> {
        self.order
            .iter()
            .filter_map(|n| self.tools.get(n))
            .map(|t| {
                z_engine_provider::ToolDef::function(
                    t.name(),
                    t.description(),
                    t.parameters_schema(),
                )
            })
            .collect()
    }

    /// Built-in toolset (grows each version).
    pub fn builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(read_file::ReadFileTool));
        reg.register(Arc::new(bash::BashTool));
        reg.register(Arc::new(write_file::WriteFileTool));
        reg.register(Arc::new(edit_file::EditFileTool));
        reg.register(Arc::new(glob::GlobTool));
        reg.register(Arc::new(grep::GrepTool));
        reg.register(Arc::new(context_notes::UpdateContextNotesTool));
        reg.register(Arc::new(task::TaskTool));

        // Read-only subset used by sub-agents (spec section 9 v0.7):
        // isolated loops default to exploration-only capabilities.

        reg
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.order)
            .finish()
    }
}
