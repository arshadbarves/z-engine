//! Engine services behind the tools crate's port traits. Subagents,
//! checks, LSP and MCP ports are wired by later phases.

mod interaction;
mod jobs;
mod side_model;
mod skills;

use std::sync::Arc;

use z_engine_tools::Ports;

use crate::session::SessionCore;

pub(crate) fn session_ports(core: &Arc<SessionCore>) -> Ports {
    Ports {
        interaction: Some(Arc::new(interaction::Interaction::new(Arc::clone(core)))),
        jobs: Some(Arc::new(jobs::Jobs::new(Arc::clone(core)))),
        skills: Some(Arc::new(skills::Skills::new(Arc::clone(core)))),
        side_model: Some(Arc::new(side_model::SideModel::new(Arc::clone(core)))),
        ..Ports::default()
    }
}
