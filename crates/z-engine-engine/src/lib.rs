//! Orchestrator of the v2 engine: session actors, agent runs, the tool
//! gate (hooks, policy, approvals), background jobs, persistence and
//! events. The desktop GUI backend drives it through [`Engine`].

mod batch;
mod broker;
mod engine;
mod error;
mod hooks;
mod options;
mod ports;
mod run;
mod session;
mod settings;
mod sync;
mod verify;

pub use engine::Engine;
pub use error::EngineError;
pub use options::{ClientFactory, EngineOptions, EventSink, ExportFormat};
