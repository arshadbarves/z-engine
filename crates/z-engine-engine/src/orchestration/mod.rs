//! Multi-agent orchestration: the agent registry, the mapping from a
//! definition to a child run, foreground and background subagents,
//! worktree isolation with apply/discard, and live `AgentInfo` tracking.

mod blueprint;
mod child;
mod decide;
mod launch;
mod orchestra;
mod recover;
mod registry;
mod report;
mod sink;
mod spawn;
mod tracker;
mod worktree;

pub(crate) use decide::{apply_command, discard_command};
pub(crate) use orchestra::Orchestra;
pub(crate) use recover::prune_stale_worktrees;
pub(crate) use registry::AgentRegistry;
pub(crate) use spawn::spawn;
pub(crate) use tracker::AgentTracker;
pub(crate) use worktree::apply;
