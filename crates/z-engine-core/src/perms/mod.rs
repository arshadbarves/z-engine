//! Permission policy engine.
//!
//! v0.1 slice of spec §5:
//!
//! - read-only tools (`read_file`, `glob`, `grep`) auto-allow;
//! - `bash` allows when **every compound segment** (`&& || ; | |& &`)
//!   either matches a command-prefix rule (`cargo test*`) or belongs to
//!   the built-in read-only allowlist (Claude Code parity: `ls`, `cat`,
//!   `grep`, read-only `git`, …). Anything unparseable fails closed;
//! - everything else gates (approval modal);
//! - "always this prefix" answers add session rules at runtime; persisted
//!   allowlists arrive in v0.5.
//!
//! A [`Decision::Gate`] answered `no` becomes a polite refusal message in
//! the conversation so the model reroutes.

mod engine;
mod read_only;
mod shell_syntax;

pub use engine::{Decision, PolicyEngine};

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(rules: &[&str]) -> PolicyEngine {
        PolicyEngine::new(rules.iter().map(|s| s.to_string()).collect())
    }

    // ---- D1: built-in read-only bash allowlist (Claude Code parity) ----

    #[test]
    fn safe_readonly_commands_auto_allow() {
        let e = engine(&[]);
        for cmd in [
            "ls -la",
            "cat src/lib.rs",
            "head -20 README.md",
            "grep -rn TODO src",
            "rg foo",
            "find . -name '*.rs'",
            "wc -l src/*.rs",
            "which cargo",
            "stat README.md",
            "du -sh .",
            "pwd",
            "echo hello",
            "printf '%s\\n' hi",
            "diff a b",
            "sort names.txt",
            "uniq -c out.txt",
            "git status",
            "git log --oneline",
            "git diff HEAD~1",
            "git show abc",
            "git blame main.rs",
            "git rev-parse HEAD",
            "git --version",
            "cargo --version",
            "cargo --list",
            "rustc --version",
            "node --version",
            "python3 --version",
        ] {
            assert_eq!(
                e.decide_command(cmd),
                Decision::Allow,
                "{cmd} should be safe"
            );
        }
    }

    #[test]
    fn mutating_network_and_unknown_commands_gate() {
        let e = engine(&[]);
        for cmd in [
            "rm -rf build",
            "mv a b",
            "cp a b",
            "mkdir sub",
            "touch f",
            "sed -i s/a/b f",
            "chmod +x run.sh",
            "curl https://example.com",
            "wget https://example.com",
            "npm install",
            "pip install requests",
            "cargo test",
            "cargo build",
            "make",
            "git push",
            "git commit -m x",
            "git checkout -b feat",
            "git clean -fd",
            "git branch -D x",
            "ssh host",
        ] {
            assert_eq!(e.decide_command(cmd), Decision::Gate, "{cmd} should gate");
        }
    }

    #[test]
    fn redirects_and_substitution_gate_even_safe_commands() {
        let e = engine(&[]);
        assert_eq!(e.decide_command("echo hi > /tmp/out"), Decision::Gate);
        assert_eq!(e.decide_command("cat < secret"), Decision::Gate);
        assert_eq!(e.decide_command("echo $(rm -rf /)"), Decision::Gate);
        assert_eq!(e.decide_command("echo `rm -rf /`"), Decision::Gate);
        // fd duplication is not a file write
        assert_eq!(e.decide_command("ls 2>&1"), Decision::Allow);
        assert_eq!(e.decide_command("grep -rn x . 2>/dev/null"), Decision::Gate);
    }
}
