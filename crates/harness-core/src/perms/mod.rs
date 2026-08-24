//! Permission policy engine.
//!
//! v0.1 slice of spec §5:
//!
//! - read-only tools (`read_file`, `glob`, `grep`) auto-allow;
//! - `bash` is checked against **command-prefix rules**: `cargo test*`
//!   prefix-matches any command starting with `cargo test`; a rule without
//!   a trailing `*` matches exactly;
//! - everything else gates (approval modal);
//! - "always this prefix" answers add session rules at runtime; persisted
//!   allowlists arrive in v0.5.
//!
//! A [`Decision::Gate`] answered `no` becomes a polite refusal message in
//! the conversation so the model reroutes.

use serde_json::Value;

/// Tools that never prompt (safe information gathering).
const AUTO_ALLOW_TOOLS: &[&str] = &[
    "read_file",
    "glob",
    "grep",
    "update_context_notes",
    // Sub-agent delegation is read-only by construction (spec section 9 v0.7).
    "task",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Gate,
}

#[derive(Debug)]
pub struct PolicyEngine {
    /// Session-scoped bash command-prefix rules (`cargo test*` style).
    bash_prefix_rules: Vec<String>,
}

impl PolicyEngine {
    pub fn new(initial_rules: Vec<String>) -> Self {
        Self {
            bash_prefix_rules: initial_rules,
        }
    }

    pub fn add_session_rule(&mut self, rule: String) {
        if !rule.trim().is_empty() && !self.bash_prefix_rules.contains(&rule) {
            self.bash_prefix_rules.push(rule);
        }
    }

    pub fn rules(&self) -> &[String] {
        &self.bash_prefix_rules
    }

    /// Decide for a tool invocation. `input` must already be a JSON object;
    /// malformed input gates (never silently allows).
    pub fn decide(&self, tool: &str, input: &Value) -> Decision {
        match tool {
            t if AUTO_ALLOW_TOOLS.contains(&t) => Decision::Allow,
            "bash" => {
                let Some(command) = input.get("command").and_then(Value::as_str) else {
                    return Decision::Gate;
                };
                self.decide_command(command)
            }
            _ => Decision::Gate,
        }
    }

    /// Rule semantics for one shell command line.
    pub fn decide_command(&self, command: &str) -> Decision {
        let command = command.trim();
        for rule in &self.bash_prefix_rules {
            if rule_matches(rule, command) {
                return Decision::Allow;
            }
        }
        Decision::Gate
    }

    /// Derive the "always this prefix" rule shown by the approval modal:
    /// first two whitespace-separated tokens plus `*` (falls back to the
    /// whole trimmed command when there's only one token).
    pub fn suggested_rule(command: &str) -> String {
        let toks: Vec<&str> = command.split_whitespace().collect();
        if toks.len() >= 2 {
            format!("{} {}*", toks[0], toks[1])
        } else {
            format!("{}*", command.trim())
        }
    }

    /// True when persisting this rule to disk would be unsafe (v0.5 uses
    /// this for outside-root operations; kept here so callers share it).
    pub const PERSIST_DISABLED: bool = false;
}

fn rule_matches(rule: &str, command: &str) -> bool {
    match rule.strip_suffix('*') {
        Some(prefix) => command.starts_with(prefix.trim_end()),
        None => command == rule,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn engine(rules: &[&str]) -> PolicyEngine {
        PolicyEngine::new(rules.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn reads_are_auto_allowed() {
        let e = engine(&[]);
        assert_eq!(e.decide("read_file", &json!({})), Decision::Allow);
        assert_eq!(e.decide("glob", &json!({})), Decision::Allow);
        assert_eq!(e.decide("grep", &json!({})), Decision::Allow);
    }

    #[test]
    fn unknown_and_missing_tools_gate() {
        let e = engine(&[]);
        assert_eq!(e.decide("write_file", &json!({})), Decision::Gate);
        assert_eq!(e.decide("bash", &json!({})), Decision::Gate); // no command key
        assert_eq!(e.decide("mystery", &json!({})), Decision::Gate);
    }

    #[test]
    fn prefix_star_rule_matches_family() {
        let e = engine(&["cargo test*"]);
        assert_eq!(
            e.decide("bash", &json!({"command": "cargo test --lib"})),
            Decision::Allow
        );
        assert_eq!(
            e.decide("bash", &json!({"command": "cargo  test"})), // extra space still starts_with after trim? No:
            Decision::Gate
        );
    }

    #[test]
    fn exact_rule_without_star() {
        let e = engine(&["git status"]);
        assert_eq!(
            e.decide("bash", &json!({"command": "git status"})),
            Decision::Allow
        );
        assert_eq!(
            e.decide("bash", &json!({"command": "git status --porcelain"})),
            Decision::Gate
        );
    }

    #[test]
    fn dangerous_commands_never_accidentally_match() {
        let e = engine(&["rm -rf /tmp/scratch*"]);
        assert_eq!(
            e.decide("bash", &json!({"command": "rm -rf /tmp/scratch/x"})),
            Decision::Allow
        );
        assert_eq!(
            e.decide("bash", &json!({"command": "rm -rf /"})),
            Decision::Gate
        );
    }

    #[test]
    fn session_rules_added_dynamically() {
        let mut e = engine(&[]);
        assert_eq!(e.decide_command("ls -la"), Decision::Gate);
        e.add_session_rule(PolicyEngine::suggested_rule("ls -la"));
        assert_eq!(e.rules(), &["ls -la*".to_string()]);
        assert_eq!(e.decide_command("ls -la"), Decision::Allow);
        // duplicate suppressed
        e.add_session_rule("ls -la*".into());
        assert_eq!(e.rules().len(), 1);
    }

    #[test]
    fn suggested_rule_uses_two_tokens() {
        assert_eq!(
            PolicyEngine::suggested_rule("cargo test --lib foo"),
            "cargo test*"
        );
        assert_eq!(PolicyEngine::suggested_rule("make"), "make*");
    }

    #[test]
    fn leading_whitespace_in_command_is_trimmed() {
        let e = engine(&["cargo build*"]);
        assert_eq!(
            e.decide_command("   cargo build --release  "),
            Decision::Allow
        );
    }
}
