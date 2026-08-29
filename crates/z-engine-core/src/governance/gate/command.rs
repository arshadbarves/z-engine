//! The shell half of the gate. A patch can be checked against evidence
//! before it lands; a shell command cannot — its write set is only known
//! after it runs. So in a guarded run only a command proven to write
//! nothing may execute, and everything else is refused with a pointer at
//! the tools that *can* be authorized.

use super::engine::{GateDecision, GateEngine};
use super::failure::GateFailure;

impl GateEngine {
    /// Shell verdict for guarded runs. `provably_read_only` comes from
    /// the permission engine's tokenizer — the gate does not parse shell,
    /// and an unparseable command must arrive here as `false`.
    pub fn authorize_command(command: &str, provably_read_only: bool) -> GateDecision {
        if provably_read_only {
            GateDecision::Pass
        } else {
            GateDecision::Fail(GateFailure::UnprovenWriteSet {
                command: command.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_commands_pass_and_unprovable_ones_are_refused() {
        assert_eq!(
            GateEngine::authorize_command("ls -la", true),
            GateDecision::Pass
        );
        let decision = GateEngine::authorize_command("cargo test", false);
        assert!(
            matches!(&decision, GateDecision::Fail(GateFailure::UnprovenWriteSet { command })
                if command == "cargo test"),
            "{decision:?}"
        );
    }
}
