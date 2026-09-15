//! Tool subsystem: the [`Tool`] trait (the only extension seam), the
//! execution context handed to every tool, the registry advertised to the
//! model, and shared output-truncation plumbing.
//!
//! Loop contract reminders (spec §4.2):
//! - tool errors are *data*: they become tool-result messages so the model
//!   can self-correct — they never crash the loop;
//! - oversized outputs are truncated head+tail for the transcript while the
//!   full text lands in a temp file whose path is embedded in the result.

pub mod assess_completion;
pub mod bash;
pub mod checkpoint;
mod checkpoint_session;
pub mod context_notes;
pub mod edit_file;
pub mod file_state;
pub mod glob;
pub mod grep;
pub mod inspect_project;
pub mod lsp_tools;
pub mod read_file;
pub mod run_verification;
pub mod task;
pub mod write_file;

mod bash_script;
mod checkpoint_restore;
mod context;
mod edit_ladder;
mod fsutil;
mod grep_backend;
mod interface;
mod proc_helpers;
mod registry;
mod shell;
#[cfg(windows)]
mod shell_detect;

pub use checkpoint::CheckpointStore;
pub use checkpoint_session::{SessionFileChange, list_session_changes, session_diff_for};
pub use context::{SubAgentFuture, SubAgentRunner, ToolCtx, ToolOutputChunk};
pub(crate) use fsutil::atomic_write;
pub use fsutil::{MAX_TOOL_OUTPUT_CHARS, truncate_with_tempfile, unified_diff};
pub use interface::{Tool, ToolError, ToolOutput};
pub use registry::ToolRegistry;

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
