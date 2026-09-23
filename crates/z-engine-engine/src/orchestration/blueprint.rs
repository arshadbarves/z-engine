//! From an agent definition and its caller to the child's `AgentSpec`:
//! the definition prompt plus the subagent preamble, the filtered tools
//! (never the user-facing ones, no `Agent` at the depth limit), the model
//! (`inherit`, a role alias, or a literal id), the permission mode, the
//! turn budget and the isolation.

use std::path::PathBuf;

use z_engine_config::{AgentDef, Settings};
use z_engine_protocol::{AgentId, Isolation};
use z_engine_tools::names;

use crate::run::{AgentSpec, ModelChoice, RunContext, ToolFilter, WorktreeScope};

/// Where the child works.
#[derive(Debug, Clone)]
pub(crate) struct Placement {
    pub root: PathBuf,
    pub worktree: Option<WorktreeScope>,
}

pub(crate) fn child_spec(
    def: &AgentDef,
    parent: &RunContext,
    agent_id: AgentId,
    placement: Placement,
) -> AgentSpec {
    let settings = parent.core.settings();
    let limits = &settings.settings.agents;
    let depth = parent.spec.depth + 1;
    let mut deny = def.disallowed_tools.clone();
    deny.extend([names::ASK_USER_QUESTION, names::EXIT_PLAN_MODE].map(String::from));
    if depth >= limits.max_depth {
        deny.push(names::AGENT.to_string());
    }
    AgentSpec {
        agent_id,
        base_prompt: format!(
            "{}\n\n{}",
            def.prompt.trim(),
            z_engine_prompts::system::SUBAGENT.trim()
        ),
        tools: ToolFilter {
            allow: def.tools.clone(),
            deny,
        },
        model: ModelChoice::Fixed(model_for(def, &settings.settings, &parent.model())),
        mode: def.permission_mode.or(parent.spec.mode),
        max_turns: def.max_turns.unwrap_or(limits.max_turns).max(1),
        root: placement.root,
        depth,
        worktree: placement.worktree,
    }
}

/// `inherit` (or nothing) takes the caller's model; `main`, `fast` and
/// `review` name the configured roles; anything else is a model id.
pub(crate) fn model_for(def: &AgentDef, settings: &Settings, parent_model: &str) -> String {
    let named = def.model.as_deref().map(str::trim).unwrap_or_default();
    match named {
        "" | "inherit" => parent_model.to_string(),
        "main" => settings.model.main.clone(),
        "fast" => settings.model.fast_model().to_string(),
        "review" => settings.model.review_model().to_string(),
        id => id.to_string(),
    }
}

/// The caller's explicit choice wins over the definition's.
pub(crate) fn isolation(def: &AgentDef, requested: Option<Isolation>) -> Isolation {
    requested.unwrap_or(def.isolation)
}

#[cfg(test)]
mod tests {
    use z_engine_config::{ExtensionScope, ExtensionSource, parse_agent};

    use super::*;

    fn def(frontmatter: &str) -> AgentDef {
        let markdown = format!("---\nname: t\ndescription: T.\n{frontmatter}---\nBody.\n");
        let source = ExtensionSource {
            scope: ExtensionScope::Project,
            path: "/p/t.md".into(),
        };
        parse_agent(&markdown, source).unwrap()
    }

    #[test]
    fn models_resolve_inherit_aliases_and_ids() {
        let mut settings = Settings::default();
        settings.model.main = "main-model".into();
        settings.model.fast = Some("fast-model".into());
        assert_eq!(model_for(&def(""), &settings, "parent"), "parent");
        assert_eq!(
            model_for(&def("model: inherit\n"), &settings, "parent"),
            "parent"
        );
        assert_eq!(
            model_for(&def("model: fast\n"), &settings, "parent"),
            "fast-model"
        );
        assert_eq!(
            model_for(&def("model: review\n"), &settings, "parent"),
            "main-model"
        );
        assert_eq!(
            model_for(&def("model: vendor/x-1\n"), &settings, "parent"),
            "vendor/x-1"
        );
    }

    #[test]
    fn requested_isolation_overrides_the_definition() {
        let worktree = def("isolation: worktree\n");
        assert_eq!(isolation(&worktree, None), Isolation::Worktree);
        assert_eq!(
            isolation(&worktree, Some(Isolation::Shared)),
            Isolation::Shared
        );
    }
}
