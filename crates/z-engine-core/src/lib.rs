//! # z-engine-core
//!
//! The "brain" of the Z Engine coding agent: agent loop, tools,
//! permissions, context engine, sessions, config. **No UI
//! dependencies** — everything here is headless and unit-testable.
//!
//! LLM transport lives in the separate `z-engine-provider` crate; core
//! depends only on its public types (dependency inversion: swap the
//! provider without touching the brain).

pub use z_engine_provider;

pub mod agent;
pub mod config;
pub mod context;
pub mod evidence;
/// Crash-safe file replacement shared by evidence, governance, and tools.
pub(crate) mod fs_atomic;
pub mod governance;
pub mod lsp;
pub mod mcp;
pub mod perms;
pub mod prompts;
pub mod replay;
pub mod session;
pub mod tools;
