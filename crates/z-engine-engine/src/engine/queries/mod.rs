//! Read-mostly queries the desktop GUI needs beside sessions: review-panel
//! diffs (session and git scope), worktrees, agent and slash-command
//! catalogs, `@file` search, and MCP server tests. Results serialize
//! camelCase for the GUI.

mod catalogs;
mod changes;
mod files;
mod git;
mod mcp;
#[cfg(test)]
mod testing;

pub use catalogs::{AgentCard, SlashCommandInfo, SlashKind};
pub use changes::{ChangedKind, ChangedPath};
pub use git::GitChangedFile;
pub use mcp::McpTestReport;
