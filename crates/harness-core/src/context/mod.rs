//! Context engine (spec §6): L0 system prompt + AGENTS.md loader, the L1
//! notes store, budget metering, and the compaction ladder.
//!
//! Stability matters: the system prompt is the provider-cache-friendly L0
//! prefix, so nothing dynamic (timestamps, trees, counters) belongs here.

pub mod budget;
pub mod compact;
pub mod notes;

use std::path::Path;

/// Operating instructions for the agent. Deliberately static text.
const BASE_INSTRUCTIONS: &str = r#"You are harness, an autonomous coding agent working inside the user's repository from a terminal.

Operating rules:
- You accomplish tasks end-to-end: read code, make edits, run commands, verify results yourself.
- Prefer precise, minimal changes that fit the existing style of the repo.
- Verify your work by running the relevant tests/build commands before declaring success.
- If a command fails, read the error and adapt; do not repeat a failing action unchanged.
- Ask nothing rhetorically: act with the tools available, then report concisely.
- File paths you pass are relative to the project root unless absolute.

Context management:
- Call `update_context_notes` every few turns with progress, firm decisions
  and things needed later. These notes survive context compaction verbatim.
- Old large tool outputs show a marker like `[harness:tool-output id=abcd1234]`;
  when you no longer need one, list its marker in `droppable` to free context."#;

/// Build the L0 system prompt. `agents_md` (if present) is appended
/// verbatim below the base instructions.
pub fn build_system_prompt(project_root: &Path, agents_md: Option<&str>) -> String {
    let mut p = String::from(BASE_INSTRUCTIONS);
    p.push_str("\n\nProject root: ");
    p.push_str(&project_root.to_string_lossy());
    if let Some(md) = agents_md {
        p.push_str("\n\n# AGENTS.md (project instructions)\n");
        p.push_str(md.trim_end());
        p.push('\n');
    }
    p
}

/// Read `<project>/AGENTS.md` when it exists (spec §6; full layering in v0.3).
pub fn load_agents_md(project_root: &Path) -> Option<String> {
    std::fs::read_to_string(project_root.join("AGENTS.md"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_root_and_instructions() {
        let p = build_system_prompt(Path::new("/tmp/proj"), None);
        assert!(p.contains("Project root: /tmp/proj"));
        assert!(p.contains("autonomous coding agent"));
    }

    #[test]
    fn agents_md_appended_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("AGENTS.md"), "Always run clippy.\n").unwrap();
        let md = load_agents_md(tmp.path()).unwrap();
        let p = build_system_prompt(tmp.path(), Some(&md));
        assert!(p.contains("# AGENTS.md"));
        assert!(p.contains("Always run clippy."));
    }

    #[test]
    fn missing_or_blank_agents_md_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_agents_md(tmp.path()).is_none());
        std::fs::write(tmp.path().join("AGENTS.md"), "   \n").unwrap();
        assert!(load_agents_md(tmp.path()).is_none());
    }
}
