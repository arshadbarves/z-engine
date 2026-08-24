//! # harness-core
//!
//! The "brain" of the harness coding agent: provider client, agent loop,
//! tools, permissions, context engine, sessions, config. **No UI
//! dependencies** — everything here is headless and unit-testable.

pub mod agent;
pub mod config;
pub mod context;
pub mod perms;
pub mod provider;
pub mod session;
pub mod tools;
