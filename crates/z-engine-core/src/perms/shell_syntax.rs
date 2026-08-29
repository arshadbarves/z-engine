//! Shell surface syntax: splitting a command line into segments and one
//! segment into tokens. Nothing here decides anything — it only reports
//! what the shell would see, and refuses (with `None`) any construct
//! whose effects cannot be read off the text: redirections, command
//! substitution, parameter expansion, unbalanced quotes.
//!
//! The read-only verdict built on top of this lives in `super::read_only`.

/// Common filesystem commands auto-approved in `accept-edits` mode
/// (Claude Code parity). Relative targets only — the project cwd is the
/// boundary; absolute/`~` paths still gate.
pub(super) const FS_MUTATING_CMDS: &[&str] = &["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"];

/// Split a command line into segments at top-level separators
/// (`&&`, `||`, `;`, `|`, `|&`, `&`, newline), respecting quotes and
/// backslash escapes. `N>&M` fd duplications are not separators.
/// `None` = unparseable (unbalanced quotes or >10k chars) → fail closed.
pub(super) fn segments(command: &str) -> Option<Vec<String>> {
    if command.len() > 10_000 {
        return None;
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        let quoted = in_single || in_double;
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                cur.push(c);
            }
            '"' if !in_single => {
                in_double = !in_double;
                cur.push(c);
            }
            '\\' if !in_single => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ';' | '\n' if !quoted => out.push(std::mem::take(&mut cur)),
            '|' if !quoted => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            '&' if !quoted => {
                // `>&` is an fd duplication, not a separator/background
                if cur.ends_with('>') {
                    cur.push(c);
                } else {
                    if chars.peek() == Some(&'&') {
                        chars.next();
                    }
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if in_single || in_double {
        return None;
    }
    out.push(cur);
    Some(out)
}

/// Quote-aware tokenizer for one segment. Returns `(command, args,
/// unquoted_glob)` or `None` when the segment redirects (`<`, `>`),
/// substitutes (`$(`, backtick), or has unbalanced quotes — all fail
/// closed. `unquoted_glob` is true when `*`/`?` appears outside quotes.
pub(super) fn tokenize(seg: &str) -> Option<(String, Vec<String>, bool)> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut unquoted_glob = false;
    let mut chars = seg.chars().peekable();
    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                cur.push(c);
            }
            continue;
        }
        if in_double {
            match c {
                '"' => in_double = false,
                // Double quotes suppress globbing and word splitting, but
                // NOT expansion: `"$(cmd)"` and "`cmd`" still execute. A
                // predicate that claims to prove a command writes nothing
                // must refuse them here exactly as it does unquoted.
                '`' | '$' => return None,
                '\\' => {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                }
                _ => cur.push(c),
            }
            continue;
        }
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            '>' => {
                // `N>&M` duplicates a file descriptor — writes nothing.
                // `>>`, `>& file`, `> path` are real writes → fail closed.
                if chars.peek() == Some(&'&') {
                    chars.next();
                    match chars.peek() {
                        Some(d) if d.is_ascii_digit() => {
                            chars.next();
                        }
                        _ => return None,
                    }
                } else {
                    return None;
                }
            }
            '<' | '`' => return None,
            // Any `$` is refused, not just `$(`: parameter expansion can
            // execute (`${v@P}` runs prompt expansion, which performs
            // command substitution) and can inject arbitrary words into
            // the argument list. Neither is provable ahead of time.
            '$' => return None,
            '\\' => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            '*' | '?' => {
                unquoted_glob = true;
                cur.push(c);
            }
            _ => cur.push(c),
        }
    }
    if in_single || in_double {
        return None;
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    let first = tokens.first()?.clone();
    Some((first, tokens.into_iter().skip(1).collect(), unquoted_glob))
}

#[cfg(test)]
mod tests {
    use super::super::{Decision, PolicyEngine};

    fn engine(rules: &[&str]) -> PolicyEngine {
        PolicyEngine::new(rules.iter().map(|s| s.to_string()).collect())
    }

    /// The tokenizer reports unquoted globs because the shell expands
    /// them before the command sees them: `find *` can arrive as
    /// `find -delete`.
    #[test]
    fn unquoted_globs_are_reported_and_gate_option_taking_commands() {
        let e = engine(&[]);
        assert_eq!(e.decide_command("find *"), Decision::Gate);
        assert_eq!(e.decide_command("sort *"), Decision::Gate);
        assert_eq!(e.decide_command("rg foo *"), Decision::Gate);
        // …but a command with no dangerous option cannot be harmed by one
        assert_eq!(e.decide_command("wc -l src/*.rs"), Decision::Allow);
        assert_eq!(e.decide_command("find . -name '*.rs'"), Decision::Allow);
    }

    #[test]
    fn unparseable_commands_fail_closed() {
        let e = engine(&[]);
        assert_eq!(e.decide_command("echo \"unterminated"), Decision::Gate);
        let huge = format!("echo {}", "x".repeat(10_001));
        assert_eq!(e.decide_command(&huge), Decision::Gate);
    }

    /// Quoting hides nothing from the shell, so it must hide nothing from
    /// the tokenizer: double quotes suppress globbing and word splitting
    /// but still expand `$(...)`, backticks, and parameters.
    #[test]
    fn expansion_inside_double_quotes_is_not_read_only() {
        let e = engine(&[]);
        for command in [
            r#"echo "$(rm -rf build)""#,
            "cat \"`rm -rf build`\"",
            r#"echo "${payload@P}""#,
            "echo ${payload@P}",
            "echo $HOME",
        ] {
            assert!(
                !PolicyEngine::is_provably_read_only(command),
                "{command} must not be provable"
            );
            assert_eq!(e.decide_command(command), Decision::Gate, "{command}");
        }
    }

    /// Redirections are writes and fd duplications are not, and the
    /// distinction has to survive tokenization.
    #[test]
    fn redirection_is_a_write_but_fd_duplication_is_not() {
        let e = engine(&[]);
        assert_eq!(e.decide_command("echo hi > out"), Decision::Gate);
        assert_eq!(e.decide_command("echo hi >> out"), Decision::Gate);
        assert_eq!(e.decide_command("cat < secret"), Decision::Gate);
        assert_eq!(e.decide_command("ls 2>&1"), Decision::Allow);
    }
}
