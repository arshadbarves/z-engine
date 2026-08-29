//! Applying the table to one tokenized segment.
//!
//! The proof has four steps and fails at any of them: the command must
//! be on the table, an unquoted glob must not be able to expand into
//! anything the entry would refuse, every option form must be proven,
//! and every operand must be one the command only reads.

use super::super::shell_syntax::tokenize;
use super::table::entry_for;

/// True when running this segment provably executes nothing and writes
/// nothing. Anything unparseable, unlisted, or merely unreviewed is
/// `false` — the answer means "proven", not "probably fine".
pub(in crate::perms) fn segment_is_safe(seg: &str) -> bool {
    let Some((command, args, unquoted_glob)) = tokenize(seg) else {
        return false;
    };
    let Some((policy, operand_rule)) = entry_for(&command) else {
        return false;
    };
    // The shell expands globs before the command sees them, so `find *`
    // can arrive as `find -delete` and `uniq *` as `uniq in out`, which
    // writes `out`. Surviving that needs both halves: no option worth
    // constraining, and no operand slot but input.
    if unquoted_glob && !(policy.options_are_inert() && operand_rule.expansion_only_adds_inputs()) {
        return false;
    }
    let Some(operands) = policy.operands(&args) else {
        return false;
    };
    operand_rule.admits(&operands)
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

    /// An operand slot is as dangerous as an option, and a command-name
    /// table cannot see it: every command below has an inert *option*
    /// surface and still writes a file or changes the machine through a
    /// bare operand.
    ///
    /// Two of these are demonstrated for real in
    /// `operand_writes_are_real_not_hypothetical`; the machine-changing
    /// ones (`hostname NAME`, `date MMDDhhmm`) are deliberately never
    /// executed — being unable to try them safely is precisely why the
    /// answer has to be proven from the text.
    #[test]
    fn operands_that_write_or_change_the_machine_are_never_proven() {
        for command in [
            // uniq(1) is `uniq [OPTION]... [INPUT [OUTPUT]]`
            "uniq in.txt out.txt",
            "uniq -c in.txt out.txt",
            "uniq --count in.txt out.txt",
            "uniq -f 1 in.txt out.txt",
            "uniq --skip-fields=1 in.txt out.txt",
            "uniq -u /etc/passwd /etc/shadow",
            // glob expansion supplies the second operand
            "uniq *",
            "uniq *.txt",
            // hostname(1) with an operand sets the system hostname
            "hostname evil.example.com",
            "hostname -s box",
            "hostname -F /etc/myname",
            "hostname -b box",
            // file(1) compiles magic files to <name>.mgc
            "file -C -m custom.magic",
            "file --compile -m custom.magic",
            "file -C",
            // and may fork a decompressor
            "file -z archive.gz",
            "file -Z archive.gz",
            // date(1) with a non-`+` operand sets the system clock
            "date 010112002026",
            "date 12312359",
            "date -u 12312359",
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

    /// The adversarial cases above are not strawmen. Run the two operand
    /// forms that can be run without touching anything outside a
    /// temporary directory, and confirm each really creates a file — so
    /// the refusals are refusing something.
    #[test]
    fn operand_writes_are_real_not_hypothetical() {
        use std::process::Command;

        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("in.txt"), b"a\na\nb\n").unwrap();
        std::fs::write(dir.join("custom.magic"), b"0\tstring\tfoo\tfoo file\n").unwrap();

        // Everything below runs inside `dir`; nothing outside it is
        // touched, and no form that changes the machine is ever run.
        let ran = |program: &str, args: &[&str]| {
            Command::new(program)
                .args(args)
                .current_dir(dir)
                .status()
                .is_ok_and(|s| s.success())
        };

        // `uniq INPUT OUTPUT` writes OUTPUT.
        let uniq_ran = ran("uniq", &["in.txt", "out.txt"]);
        if uniq_ran {
            assert!(
                dir.join("out.txt").is_file(),
                "uniq's second operand is an output file"
            );
        }
        #[cfg(unix)]
        assert!(uniq_ran, "uniq(1) is needed to demonstrate the write");

        // `file -C -m NAME` compiles NAME into NAME.mgc.
        if ran("file", &["-C", "-m", "custom.magic"]) {
            assert!(
                dir.join("custom.magic.mgc").is_file(),
                "file -C writes a compiled magic file"
            );
        }

        // …and the predicate's answer about them does not depend on
        // whether those binaries happen to exist here.
        assert!(!PolicyEngine::is_provably_read_only("uniq in.txt out.txt"));
        assert!(!PolicyEngine::is_provably_read_only(
            "file -C -m custom.magic"
        ));
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
            "date +%Y-%m-%d",
            "date -u +%s",
            "date -d yesterday +%F",
            "uniq names.txt",
            "uniq -c names.txt",
            "uniq -f 1 -i names.txt",
            "hostname",
            "hostname -s",
            "file README.md",
            "file -b --mime-type README.md",
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
