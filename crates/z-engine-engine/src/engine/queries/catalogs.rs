//! What the composer offers: agents for `@` mentions and slash commands.
//! Slash commands are the ones the engine handles today plus the GUI's
//! own; prompt and custom commands arrive with the commands phase.

use std::path::Path;

use serde::Serialize;
use z_engine_config::{
    AgentDef, ExtensionScope, ExtensionSource, discover_extensions, parse_agent,
};
use z_engine_prompts::agents::BUILTIN;

use crate::engine::Engine;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    /// `builtin`, `user` or `project` (`.claude/` folders count as their level).
    pub source: String,
    pub model: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SlashKind {
    /// Sent to the session as `runCommand`.
    Engine,
    /// Handled by the GUI.
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommandInfo {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    pub source: String,
    pub kind: SlashKind,
}

/// `(name, description, argument hint)` of the commands in `session/slash.rs`.
const ENGINE_COMMANDS: &[(&str, &str, Option<&str>)] = &[
    (
        "compact",
        "Summarize older history to free context",
        Some("[instructions]"),
    ),
    ("cost", "Show token usage and cost of this chat", None),
    ("status", "Show model, mode, and session status", None),
    (
        "remember",
        "Save a note to AGENTS.md",
        Some("<project|local|user> <text>"),
    ),
];

const UI_COMMANDS: &[(&str, &str, Option<&str>)] = &[
    ("help", "Show commands and keyboard shortcuts", None),
    ("agents", "Open the agents panel", None),
    ("jobs", "Open background jobs", None),
    ("mcp", "Manage MCP servers", None),
    ("permissions", "Manage permission rules", None),
    ("hooks", "Show recent hook runs", None),
    ("memory", "Remember something in AGENTS.md", None),
    ("config", "Open settings", None),
    ("resume", "Switch to another chat", None),
    (
        "export",
        "Copy the transcript to the clipboard",
        Some("[markdown|json]"),
    ),
    ("clear", "Start a new chat", None),
    ("context", "Show context usage by prompt layer", None),
];

impl Engine {
    /// Built-in agents, then custom agents of `project_root` (a custom
    /// agent replaces a built-in of the same name), sorted by name.
    pub fn agent_cards(&self, project_root: &Path) -> Vec<AgentCard> {
        let compat = self.settings(Some(project_root)).settings.compat.claude;
        let custom = discover_extensions(self.paths(), project_root, compat).agents;
        let builtin = BUILTIN.iter().filter_map(|(name, markdown)| {
            let source = ExtensionSource::new(ExtensionScope::User, Path::new(name));
            parse_agent(markdown, source)
                .inspect_err(|error| tracing::warn!(%name, %error, "built-in agent skipped"))
                .ok()
                .map(|def| card(def, "builtin"))
        });
        let mut cards: Vec<AgentCard> = builtin
            .filter(|card| custom.iter().all(|def| def.name != card.name))
            .collect();
        cards.extend(custom.into_iter().map(|def| {
            let source = match def.source.scope {
                ExtensionScope::ClaudeUser | ExtensionScope::User => "user",
                ExtensionScope::ClaudeProject | ExtensionScope::Project => "project",
            };
            card(def, source)
        }));
        cards.sort_by(|a, b| a.name.cmp(&b.name));
        cards
    }

    /// Slash commands for `project_root`: engine-handled, then GUI ones.
    pub fn slash_commands(&self, _project_root: &Path) -> Vec<SlashCommandInfo> {
        let list = |commands: &[(&str, &str, Option<&str>)], kind| {
            commands
                .iter()
                .map(move |(name, description, hint)| SlashCommandInfo {
                    name: name.to_string(),
                    description: description.to_string(),
                    argument_hint: hint.map(str::to_string),
                    source: "builtin".to_string(),
                    kind,
                })
                .collect::<Vec<_>>()
        };
        let mut commands = list(ENGINE_COMMANDS, SlashKind::Engine);
        commands.extend(list(UI_COMMANDS, SlashKind::Ui));
        commands
    }
}

fn card(def: AgentDef, source: &str) -> AgentCard {
    AgentCard {
        name: def.name,
        description: def.description,
        source: source.to_string(),
        model: def.model,
        color: def.color,
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::engine;
    use super::*;

    #[test]
    fn builtin_agents_are_listed_and_custom_ones_override_them() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("project");
        let agents = root.join(".z-engine").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(
            agents.join("explore.md"),
            "---\ndescription: Local explorer\ncolor: teal\nmodel: fast\n---\nLook around.\n",
        )
        .unwrap();
        let cards = engine.agent_cards(&root);
        let names: Vec<&str> = cards.iter().map(|card| card.name.as_str()).collect();
        assert_eq!(names, ["explore", "general", "plan", "review", "verify"]);
        let explore = &cards[0];
        assert_eq!(explore.source, "project");
        assert_eq!(explore.description, "Local explorer");
        assert_eq!(explore.color.as_deref(), Some("teal"));
        assert!(cards[1..].iter().all(|card| card.source == "builtin"));
        let json = serde_json::to_value(explore).unwrap();
        assert!(json.get("description").is_some() && json.get("model").is_some());
    }

    #[test]
    fn slash_commands_are_the_engine_and_gui_ones() {
        let dir = tempfile::tempdir().unwrap();
        let commands = engine(dir.path()).slash_commands(dir.path());
        let engine_names: Vec<&str> = commands
            .iter()
            .filter(|command| command.kind == SlashKind::Engine)
            .map(|command| command.name.as_str())
            .collect();
        assert_eq!(engine_names, ["compact", "cost", "status", "remember"]);
        assert_eq!(commands.len(), ENGINE_COMMANDS.len() + UI_COMMANDS.len());
        let json = serde_json::to_value(&commands[0]).unwrap();
        assert_eq!(json["argumentHint"], "[instructions]");
        assert_eq!(json["kind"], "engine");
        assert_eq!(json["source"], "builtin");
    }
}
