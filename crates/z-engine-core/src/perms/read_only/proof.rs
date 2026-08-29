//! Applying the table to one tokenized segment.
//!
//! The proof has three steps and fails at any of them: the command must
//! have a policy, an unquoted glob must not be able to expand into an
//! option the policy would refuse, and every argument must be admitted.

use super::super::shell_syntax::tokenize;
use super::table::policy_for;

/// True when running this segment provably executes nothing and writes
/// nothing. Anything unparseable, unlisted, or merely unreviewed is
/// `false` — the answer means "proven", not "probably fine".
pub(in crate::perms) fn segment_is_safe(seg: &str) -> bool {
    let Some((command, args, unquoted_glob)) = tokenize(seg) else {
        return false;
    };
    let Some(policy) = policy_for(&command) else {
        return false;
    };
    // The shell expands globs before the command sees them, so `find *`
    // can become `find -delete`. Only a command with no constrained
    // options at all survives that.
    if unquoted_glob && !policy.is_option_free() {
        return false;
    }
    policy.admits(&args)
}

#[cfg(test)]
mod tests {
    use super::super::super::{Decision, PolicyEngine};
    use super::*;

    fn engine() -> PolicyEngine {
        PolicyEngine::new(Vec::new())
    }

    /// Every one of these runs a program or writes a file through a
    /// command the old name-only allowlist called "read-only". The proof
    /// must refuse them, and — because `decide_command` shares the
    /// predicate — they must reach the approval prompt rather than
    /// auto-approving in ordinary runs.
    #[test]
    fn options_that_execute_a_program_are_never_proven() {
        for command in [
            // ripgrep runs its preprocessor for every candidate file
            "rg --pre rm foo",
            "rg --pre=rm foo",
            "rg --pre-glob *.gz --pre rm foo",
            "rg -z foo",
            "rg --search-zip foo",
            // bat pipes its output through an arbitrary command
            "bat --pager rm README.md",
            "bat --pager=rm README.md",
            // sort forks a compressor for its temporary files
            "sort --compress-program rm big.txt",
            "sort --compress-program=rm big.txt",
            "sort --random-source=/dev/zero f",
            // git-level options run configured programs
            "git -c core.pager=rm log",
            "git log --ext-diff",
            "git log --textconv",
            // and the classics
            "find . -exec rm {} ;",
            "find . -okdir rm {} ;",
            "env rm -rf build",
        ] {
            assert!(
                !PolicyEngine::is_provably_read_only(command),
                "{command} must not be provable"
            );
            assert_eq!(
                engine().decide_command(command),
                Decision::Gate,
                "{command}"
            );
        }
    }

    /// Same for options that write a file: proving a command writes
    /// nothing means proving it cannot be *told* to write.
    #[test]
    fn options_that_write_a_file_are_never_proven() {
        for command in [
            "sort -o /etc/passwd in.txt",
            "sort -o/etc/passwd in.txt",
            "sort --output=/etc/passwd in.txt",
            "tree -o listing.txt",
            "tree -olisting.txt",
            "git log --output=out.txt",
            "git log --output out.txt",
            "find . -fprint /etc/x",
            "find . -fprintf out.txt %p",
            "find . -fls listing.txt",
            "find . -delete",
            "date -s 2020-01-01",
            "date --set=2020-01-01",
        ] {
            assert!(
                !PolicyEngine::is_provably_read_only(command),
                "{command} must not be provable"
            );
            assert_eq!(
                engine().decide_command(command),
                Decision::Gate,
                "{command}"
            );
        }
    }

    /// Default-deny is the point: an option nobody has ruled on is
    /// refused *because* nobody has ruled on it, so tomorrow's
    /// `--run-this` needs no patch here.
    #[test]
    fn unreviewed_options_are_refused_without_being_named() {
        for command in [
            "rg --hostname-bin whoami foo",
            "bat --terminal-width-invented 3 f",
            "sort --files0-from list",
            "git log --invented-option",
            "find . -invented",
        ] {
            assert!(
                !PolicyEngine::is_provably_read_only(command),
                "{command} must not be provable"
            );
        }
    }

    /// The proof still has to be useful: everyday read-only work stays
    /// auto-approved, including forms the scanner has to parse properly
    /// (bundles, attached and separated values, `--`, quoted globs).
    #[test]
    fn ordinary_read_only_work_still_proves() {
        for command in [
            "ls -la",
            "cat -n src/lib.rs",
            "head -20 README.md",
            "head -n 20 README.md",
            "grep -rn TODO src",
            "rg -n --glob !target -A 3 parse",
            "rg -e parse -t rust",
            "bat --plain --paging=never README.md",
            "sort -k2,3 -t, names.txt",
            "sort -u names.txt",
            "tree -L 2",
            "find . -name '*.rs' -maxdepth 3 -print",
            "find . -mtime -1",
            "wc -l src/*.rs",
            "git status --porcelain",
            "git log --oneline -10",
            "git log -n 5 --stat",
            "git diff --cached --name-only",
            "git rev-parse --abbrev-ref HEAD",
            "git ls-files --others --exclude-standard",
            "git blame -L 10,20 main.rs",
            "git --version",
            "cargo --version",
            "cargo --list",
            "env",
            "env FOO=1",
            "date -u",
            "diff -u a b",
            "jq -r .name package.json",
            "ls -- -weird-name",
        ] {
            assert!(
                PolicyEngine::is_provably_read_only(command),
                "{command} must stay provable"
            );
            assert_eq!(
                engine().decide_command(command),
                Decision::Allow,
                "{command}"
            );
        }
    }

    #[test]
    fn unlisted_commands_and_unparseable_lines_prove_nothing() {
        for command in [
            "curl https://example.com",
            "xargs rm",
            "nice rm -rf build",
            "timeout 5 rm x",
            "echo \"unterminated",
            "git",
            "cargo",
        ] {
            assert!(!segment_is_safe(command), "{command}");
        }
    }
}
