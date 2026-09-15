//! Effect proof is deliberately narrower than permission to execute a command.

use super::shell_syntax::{segments, tokenize};

pub(super) fn command_is_read_only(command: &str) -> bool {
    if command.contains(['$', '`', '>', '<']) {
        return false;
    }
    segments(command).is_some_and(|parts| {
        !parts.is_empty()
            && parts.iter().all(|part| {
                let Some((name, args, _)) = tokenize(part) else {
                    return false;
                };
                match name.as_str() {
                    "pwd" => args.iter().all(|arg| matches!(arg.as_str(), "-L" | "-P")),
                    "echo" | "printf" | "ls" | "cat" | "head" | "tail" | "wc" => true,
                    "env" => args.is_empty(),
                    // find/env can launch commands; sort/date can write. Git may
                    // invoke configured external diff drivers or pagers.
                    _ => false,
                }
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_allowance_is_not_read_only_effect_evidence() {
        for command in [
            "env touch ../file",
            "env --split-string='touch ../file'",
            "find . -exec touch ../file \\;",
            "sort input -o ../file",
            "date --set=2026-01-01",
            "git diff",
            "echo \"$(touch ../file)\"",
            "echo \"$COMMAND\"",
            "printf data > ../file",
            "pwd; env touch ../file",
        ] {
            assert!(!command_is_read_only(command), "{command}");
        }
    }

    #[test]
    fn bounded_output_only_commands_remain_available() {
        for command in [
            "pwd",
            "pwd -P",
            "ls -la",
            "cat 'src/lib.rs'",
            "head -20 a | wc -l",
            "env",
        ] {
            assert!(command_is_read_only(command), "{command}");
        }
    }
}
