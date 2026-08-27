//! # harness-core
//!
//! The "brain" of the harness coding agent: agent loop, tools,
//! permissions, context engine, sessions, config. **No UI
//! dependencies** — everything here is headless and unit-testable.
//!
//! LLM transport lives in the separate [`harness-provider`] crate; core
//! depends only on its public types (dependency inversion: swap the
//! provider without touching the brain).

pub use harness_provider;

pub mod agent;
pub mod config;
pub mod context;
pub mod lsp;
pub mod mcp;
pub mod perms;
pub mod prompts;
pub mod session;
pub mod tools;
