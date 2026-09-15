//! Deterministic Rust verification, durable evidence, and completion gating.

mod artifacts;
mod cargo_scope;
mod environment;
mod evidence;
mod gate;
mod process;
mod runner;
mod snapshot;
mod spec;
mod toolchain;
mod types;

pub use evidence::blocked_evidence;
pub use gate::{assess, refresh};
pub use runner::run_check;
pub use snapshot::WorkspaceSnapshot;
pub use types::*;

#[cfg(test)]
mod gate_tests;
#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod test_workspace;
