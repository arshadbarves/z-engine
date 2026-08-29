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
pub mod lsp;
pub mod mcp;
pub mod perms;
pub mod prompts;
pub mod session;
pub mod tools;
