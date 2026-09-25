//! Orchestrator of the v2 engine: session actors, agent runs, the tool
//! gate (hooks, policy, approvals), background jobs, persistence and
//! events. The desktop GUI backend drives it through [`Engine`].

mod batch;
mod broker;
mod commands;
mod engine;
mod error;
mod hooks;
mod lsp;
mod mcp;
mod options;
mod orchestration;
mod ports;
mod run;
mod session;
mod settings;
mod sync;
mod verify;

pub use engine::{
    AgentCard, ChangedKind, ChangedPath, Engine, GitChangedFile, McpTestReport, RepoSummary,
    SlashCommandInfo, SlashKind, TrustReport,
};
pub use error::EngineError;
pub use options::{ClientFactory, EngineOptions, EventSink, ExportFormat};
