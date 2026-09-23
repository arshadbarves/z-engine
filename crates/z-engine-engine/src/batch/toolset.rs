//! The tools one run offers: the session registry filtered by the agent
//! definition, main-agent-only tools, `Agent` above the nesting limit, and
//! services this phase does not wire (checks, LSP, MCP resources) or
//! settings disable.

use std::sync::Arc;

use z_engine_host::SearchBackend;
use z_engine_llm::ToolSpec;
use z_engine_tools::{Tool, names};

use crate::run::RunContext;

#[derive(Clone, Default)]
pub(crate) struct ToolSet {
    tools: Vec<Arc<dyn Tool>>,
}

impl std::fmt::Debug for ToolSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}

impl ToolSet {
    pub(crate) fn offered(ctx: &RunContext) -> Self {
        let settings = ctx.core.settings();
        let offer = Offer {
            main: ctx.spec.is_main(),
            may_spawn: ctx.spec.depth < settings.settings.agents.max_depth,
            search: &settings.web_search,
        };
        let registry = ctx.core.tools();
        let tools = registry
            .iter()
            .filter(|tool| {
                let name = tool.name();
                ctx.spec.tools.permits(name) && offer.available(name)
            })
            .cloned()
            .collect();
        Self { tools }
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name)
    }

    pub(crate) fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|tool| tool.name()).collect()
    }

    pub(crate) fn specs(&self) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .map(|tool| ToolSpec {
                name: tool.name().to_string(),
                description: tool.description(),
                input_schema: tool.input_schema(),
            })
            .collect()
    }
}

/// What decides a tool's availability beyond the agent's filter.
struct Offer<'a> {
    main: bool,
    /// The agent is above the nesting limit, so its children may exist.
    may_spawn: bool,
    search: &'a SearchBackend,
}

impl Offer<'_> {
    /// Worktree changes merge into the project tree, which only the main
    /// agent owns.
    fn available(&self, name: &str) -> bool {
        match name {
            names::VERIFY | names::LSP | names::LIST_MCP_RESOURCES | names::READ_MCP_RESOURCE => {
                false
            }
            names::AGENT => self.may_spawn,
            names::ASK_USER_QUESTION | names::EXIT_PLAN_MODE | names::APPLY_AGENT_CHANGES => {
                self.main
            }
            names::WEB_SEARCH => *self.search != SearchBackend::None,
            _ => true,
        }
    }
}
