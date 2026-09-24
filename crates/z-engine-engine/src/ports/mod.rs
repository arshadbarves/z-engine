//! Engine services behind the tools crate's port traits.

mod agents;
mod checks;
mod interaction;
mod jobs;
mod lsp;
mod mcp;
mod side_model;
mod skills;

use std::sync::Arc;

use z_engine_tools::{LspPort, Ports};

use crate::run::RunContext;

/// The ports of one run's tool calls; subagents spawn as its children.
pub(crate) fn run_ports(ctx: &RunContext) -> Ports {
    let core = &ctx.core;
    let lsp = core
        .lsp
        .worker()
        .map(|worker| Arc::new(lsp::Lsp::new(worker)) as Arc<dyn LspPort>);
    Ports {
        agents: Some(Arc::new(agents::Agents::new(ctx.clone()))),
        interaction: Some(Arc::new(interaction::Interaction::new(Arc::clone(core)))),
        jobs: Some(Arc::new(jobs::Jobs::new(Arc::clone(core)))),
        skills: Some(Arc::new(skills::Skills::new(Arc::clone(core)))),
        checks: Some(Arc::new(checks::Checks::new(Arc::clone(core)))),
        lsp,
        mcp: Some(Arc::new(mcp::Mcp::new(
            Arc::clone(core),
            Arc::clone(&ctx.mcp),
        ))),
        side_model: Some(Arc::new(side_model::SideModel::new(Arc::clone(core)))),
    }
}
