/// Built-in read-only bash commands (Claude Code's "read-only commands"
/// set): never prompt, in every mode. First-token match.
const SAFE_BASH: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "find",
    "wc",
    "which",
    "type",
    "file",
    "stat",
    "du",
    "df",
    "pwd",
    "echo",
    "printf",
    "cd",
    "diff",
    "cmp",
    "comm",
    "sort",
    "uniq",
    "tree",
    "env",
    "printenv",
    "id",
    "whoami",
    "hostname",
    "uname",
    "date",
    "seq",
    "expr",
    "test",
    "[",
    "fmt",
    "pr",
    "numfmt",
    "tsort",
    "getconf",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "md5sum",
    "shasum",
    "sha256sum",
    "cksum",
    "nl",
    "bat",
    "jq",
];

/// `git` subcommands considered read-only.
const SAFE_GIT_SUBCOMMANDS: &[&str] = &[
    "status",
    "log",
    "diff",
    "show",
    "blame",
    "rev-parse",
    "describe",
    "ls-files",
];

/// Toolchain version probes: `<cmd> --version` style, read-only.
const VERSION_PROBE_CMDS: &[&str] = &[
    "cargo", "rustc", "node", "npm", "python3", "python", "go", "rustup",
];

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
            '$' if chars.peek() == Some(&'(') => return None,
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

/// Built-in read-only verdict for one segment (no separators left).
pub(super) fn segment_is_safe(seg: &str) -> bool {
    let Some((first, args, unquoted_glob)) = tokenize(seg) else {
        return false;
    };
    // An unquoted glob could expand to a write-capable flag
    // (`find *` → `find -delete`).
    if unquoted_glob && matches!(first.as_str(), "find" | "sort") {
        return false;
    }
    if first == "find"
        && args.iter().any(|a| {
            matches!(
                a.as_str(),
                "-delete" | "-exec" | "-execdir" | "-ok" | "-okdir"
            )
        })
    {
        return false;
    }
    if first == "sort"
        && args
            .iter()
            .any(|a| a == "-o" || a == "--output" || a.starts_with("--output="))
    {
        return false;
    }
    if first == "git" {
        return args
            .first()
            .is_some_and(|sub| SAFE_GIT_SUBCOMMANDS.contains(&sub.as_str()) || sub == "--version");
    }
    if VERSION_PROBE_CMDS.contains(&first.as_str())
        && !args.is_empty()
        && args
            .iter()
            .all(|a| matches!(a.as_str(), "--version" | "-V"))
    {
        return true;
    }
    if first == "cargo" && !args.is_empty() && args.iter().all(|a| a == "--list") {
        return true;
    }
    SAFE_BASH.contains(&first.as_str())
}

#[cfg(test)]
mod tests {
    use super::super::{Decision, PolicyEngine};

    fn engine(rules: &[&str]) -> PolicyEngine {
        PolicyEngine::new(rules.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn write_capable_flags_and_globs_gate() {
        let e = engine(&[]);
        assert_eq!(e.decide_command("find . -delete"), Decision::Gate);
        assert_eq!(
            e.decide_command("find . -name x -exec rm {} \\;"),
            Decision::Gate
        );
        assert_eq!(
            e.decide_command("sort -o /etc/passwd in.txt"),
            Decision::Gate
        );
        // unquoted glob could expand to a flag for write-capable commands
        assert_eq!(e.decide_command("find *"), Decision::Gate);
        assert_eq!(e.decide_command("sort *"), Decision::Gate);
        // globs are fine for plain readers
        assert_eq!(e.decide_command("wc -l src/*.rs"), Decision::Allow);
    }

    #[test]
    fn unparseable_commands_fail_closed() {
        let e = engine(&[]);
        assert_eq!(e.decide_command("echo \"unterminated"), Decision::Gate);
        let huge = format!("echo {}", "x".repeat(10_001));
        assert_eq!(e.decide_command(&huge), Decision::Gate);
    }
}
