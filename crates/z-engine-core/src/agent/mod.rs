//! The agent loop: turn orchestration, streaming consumption, permission
//! gating, tool execution, and cancellation (spec §4.2).
//!
//! Ownership model:
//! - one background tokio task owns the conversation and the loop;
//! - the UI world talks to it through [`AgentHandle`] (`Command`s in) and
//!   an [`EventRx`] (`Event`s out) — the TUI never touches tools/provider;
//! - aborts are cooperative: an atomic flag checked by the provider stream
//!   and every tool, plus `select!` points on the command channel between
//!   chunks and while awaiting approvals.
//!
//! Layout: each concern lives in a dedicated sibling module; this file is
//! the composition root only (module declarations + public re-exports).

pub mod events;

mod config;
mod execute;
mod handle;
mod prompt_inspect;
mod revert;
mod side_requests;
mod state;
mod stream;
mod subagent;
mod system_prompt;
mod task;
mod turn;

pub use config::LoopConfig;
pub use events::{ApprovalDecision, Command, Event, PermissionMode};
pub use handle::{
    AgentHandle, EventRx, ResumeState, spawn, spawn_with_provider, spawn_with_recorder,
};
pub use prompt_inspect::PromptInspect;
