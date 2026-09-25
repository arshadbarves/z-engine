//! Read-mostly queries the desktop GUI needs beside sessions: review-panel
//! diffs (session and git scope), the sidebar's repository summary,
//! worktrees, the context card's token breakdown, agent and slash-command
//! catalogs, `@file` search, MCP server tests, and workspace trust.
//! Results serialize camelCase for the GUI.

mod catalogs;
mod changes;
mod context;
mod files;
mod git;
mod mcp;
#[cfg(test)]
mod testing;
mod trust;

pub use catalogs::{AgentCard, SlashCommandInfo, SlashKind};
pub use changes::{ChangedKind, ChangedPath};
pub use git::{GitChangedFile, RepoSummary};
pub use mcp::McpTestReport;
pub use trust::TrustReport;
