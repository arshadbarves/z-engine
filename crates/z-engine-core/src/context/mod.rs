//! Context engine (spec §6): L0 system prompt + AGENTS.md loader, the L1
//! notes store, budget metering, and the compaction ladder.
//!
//! Stability matters: the system prompt is the provider-cache-friendly L0
//! prefix, so nothing dynamic (timestamps, trees, counters) belongs here.

pub mod budget;
pub mod compact;
pub mod cost;
pub mod notes;
pub mod repo_map;
mod system;
pub mod task_packet;

pub use system::{build_system_prompt, load_agents_md};
