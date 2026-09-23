//! Engine services behind the tools crate's port traits. Checks, LSP and
//! MCP ports are wired by later phases.

mod agents;
mod interaction;
mod jobs;
mod side_model;
mod skills;

use std::sync::Arc;

use z_engine_tools::Ports;

use crate::run::RunContext;

/// The ports of one run's tool calls; subagents spawn as its children.
pub(crate) fn run_ports(ctx: &RunContext) -> Ports {
    let core = &ctx.core;
    Ports {
        agents: Some(Arc::new(agents::Agents::new(ctx.clone()))),
        interaction: Some(Arc::new(interaction::Interaction::new(Arc::clone(core)))),
        jobs: Some(Arc::new(jobs::Jobs::new(Arc::clone(core)))),
        skills: Some(Arc::new(skills::Skills::new(Arc::clone(core)))),
        side_model: Some(Arc::new(side_model::SideModel::new(Arc::clone(core)))),
        ..Ports::default()
    }
}
