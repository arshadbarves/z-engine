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

/// Expansions that rewrite the *word list* before the command runs and
/// whose result cannot be read off the text. Brace expansion turns one
/// word into many (`sort {-o,out} in` → `sort -o out in`, a write), and
/// bracket expressions are pathname expansion by another name. Both are
/// refused outright rather than merely flagged, because a proof built on
/// the pre-expansion text would be proving the wrong command.
const WORD_LIST_EXPANSION: &[char] = &['{', '}', '[', ']'];

/// Quote-aware tokenizer for one segment. Returns `(command, args,
/// unquoted_glob)` or `None` when the segment redirects (`<`, `>`),
/// substitutes (`$(`, backtick), expands its own word list (`{a,b}`,
/// `[a-z]`, `~`), or has unbalanced quotes — all fail closed.
/// `unquoted_glob` is true when `*`/`?` appears outside quotes.
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
            // Brace and bracket expansion inject words the same way, and
            // tilde expansion escapes the project root. `*`/`?` are the
            // only expansions this tokenizer reports instead of refusing,
            // because the read-only table reasons about what a glob can
            // expand into (see `read_only::proof`); nothing reasons about
            // `{-o,out}`, so it must never reach the proof at all.
            c if WORD_LIST_EXPANSION.contains(&c) => return None,
            // A tilde only expands where the shell expands it: at the
            // start of a word, or just after an unquoted `=`/`:` in a
            // word that looks like an assignment. Anywhere else it is an
            // ordinary character, and refusing it there would cost
            // everyday forms like `git diff HEAD~1` for nothing.
            '~' if expands_as_tilde(&cur) => return None,
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

/// Would a `~` appearing after `word_so_far` be a tilde-prefix? True at
/// the start of a word and after the `=`/`:` of an assignment-shaped one
/// (`FOO=~/x`, `PATH=a:~/b`, `--output=~/out`), which is everywhere the
/// shell substitutes a home directory.
fn expands_as_tilde(word_so_far: &str) -> bool {
    word_so_far.is_empty() || word_so_far.ends_with('=') || word_so_far.ends_with(':')
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

    /// Brace expansion rewrites the word list *before* the command runs,
    /// so a proof read off the pre-expansion text proves nothing about
    /// the command that actually executes. Each line below is a command
    /// the option/operand tables would otherwise call read-only.
    #[test]
    fn brace_expansion_is_never_proven_because_it_forges_the_word_list() {
        let e = engine(&[]);
        for command in [
            // `sort -o out in` — writes `out`
            "sort {-o,out} in",
            // `rg --pre rm foo` — runs `rm` as ripgrep's preprocessor
            "rg {--pre,rm} foo",
            // `uniq in out` — writes `out`, defeating InputsAtMost(1)
            "uniq {in,out}",
            // `env FOO=b cat /etc/passwd` — defeats ArgPolicy::Assignments
            "env {FOO=b,cat,/etc/passwd}",
            // ranges and nesting expand just as freely
            "wc -l file{1..9}",
            "ls {a,b}/{c,d}",
            // a lone brace is still brace syntax to the shell
            "cat }",
            "cat {",
        ] {
            assert!(
                !PolicyEngine::is_provably_read_only(command),
                "{command} must not be provable"
            );
            assert_eq!(e.decide_command(command), Decision::Gate, "{command}");
        }
    }

    /// Bracket expressions are pathname expansion, and tilde expansion
    /// reaches outside the project root; neither is decidable from the
    /// text, so both refuse rather than being flagged like `*`.
    #[test]
    fn bracket_and_tilde_expansion_are_never_proven() {
        let e = engine(&[]);
        for command in [
            "uniq [io]n",
            "sort -[o]",
            "cat file[12]",
            "ls ~",
            "cat ~/.ssh/id_rsa",
            "wc -l ~user/notes",
            "sort --output=~/out in",
            "env FOO=~/x",
            "git log --grep=x --author=~y",
        ] {
            assert!(
                !PolicyEngine::is_provably_read_only(command),
                "{command} must not be provable"
            );
            assert_eq!(e.decide_command(command), Decision::Gate, "{command}");
        }
        // A tilde the shell would not expand is an ordinary character,
        // and the everyday forms that rely on that keep working.
        for command in ["git diff HEAD~1", "git log HEAD~3..HEAD", "wc -l notes~"] {
            assert!(
                PolicyEngine::is_provably_read_only(command),
                "{command} must stay provable"
            );
        }
    }

    /// Quoting is what makes these characters literal, and the tokenizer
    /// has to agree with the shell about that — otherwise the refusal
    /// above would cost every legitimate use of a brace or a bracket.
    #[test]
    fn quoted_braces_brackets_and_tildes_stay_literal_operands() {
        let e = engine(&[]);
        for command in [
            "rg -n '\\{a,b\\}' src",
            "find . -name '[a-z]*.rs'",
            "grep -rn \"{}\" src",
            "jq -r '{name: .name}' package.json",
            "wc -l 'notes~'",
        ] {
            assert!(
                PolicyEngine::is_provably_read_only(command),
                "{command} must stay provable"
            );
            assert_eq!(e.decide_command(command), Decision::Allow, "{command}");
        }
    }

    /// The stricter tokenizer must only ever tighten: an expansion the
    /// proof now refuses must not become auto-approvable through the
    /// accept-edits filesystem set either.
    #[test]
    fn expansion_syntax_also_tightens_the_accept_edits_set() {
        for command in [
            "rm {a,b}",
            "touch ~/x",
            "mv a[12] b",
            "cp {a,b} c",
            "mkdir -p {a,b}/c",
        ] {
            assert!(
                !PolicyEngine::is_common_fs_command(command),
                "{command} must not auto-approve in accept-edits"
            );
        }
        // …while the ordinary relative forms still do.
        assert!(PolicyEngine::is_common_fs_command("mkdir -p a/b"));
    }
}
