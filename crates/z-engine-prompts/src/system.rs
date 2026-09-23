//! Main system prompt layers.

/// Main agent: identity, communication, task method, tool policy, safety,
/// and verification rules.
pub const MAIN: &str = include_str!("../prompts/system/main.md");

/// Appended to every subagent's own prompt: reporting to the parent, no
/// contact with the user, scope, safety, and worktree isolation.
pub const SUBAGENT: &str = include_str!("../prompts/system/subagent.md");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_not_empty() {
        for prompt in [MAIN, SUBAGENT] {
            assert!(!prompt.trim().is_empty());
        }
    }
}
