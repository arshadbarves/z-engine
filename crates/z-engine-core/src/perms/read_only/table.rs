//! The table of commands that may appear in a read-only proof, and the
//! option surface proven inert for each. Data only — `scan` interprets
//! it, `proof` applies it.
//!
//! Adding an entry is a claim that someone read that command's manual and
//! found no way for the listed forms to execute a program or write a
//! file. `Inert` is the strongest such claim (the *whole* option surface
//! is safe) and is only used where the surface is small and settled;
//! anything with a `--pager`/`--output`/`-exec`-shaped option gets an
//! explicit list, so its dangerous forms are refused by omission.

use super::options::{
    BAT_OPTS, DATE_OPTS, FIND_PRIMARIES, GIT_OPTS, GIT_SUBCOMMANDS, RG_OPTS, SORT_OPTS, TREE_OPTS,
};
use super::scan::ArgPolicy;

/// The policy for `command`, or `None` when it may never appear in a
/// proof.
pub(super) fn policy_for(command: &str) -> Option<&'static ArgPolicy> {
    if INERT.contains(&command) {
        return Some(&INERT_POLICY);
    }
    TABLE
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, policy)| policy)
}

static INERT_POLICY: ArgPolicy = ArgPolicy::Inert;

/// Commands with no option that can execute or write, verified against
/// their documented option surface. Their arguments are inert data.
const INERT: &[&str] = &[
    // coreutils readers: every option is a formatting or selection knob.
    "ls",
    "cat",
    "head",
    "tail",
    "wc",
    "nl",
    "fmt",
    "pr",
    "numfmt",
    "tsort",
    "uniq",
    "comm",
    "cmp",
    "seq",
    "expr",
    "test",
    "[",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "printenv",
    "id",
    "whoami",
    "hostname",
    "uname",
    "getconf",
    "pwd",
    "echo",
    "printf",
    "cd",
    "which",
    "type",
    "stat",
    "du",
    "df",
    "file", // hashers: read and print, no output-file option
    "md5sum",
    "shasum",
    "sha256sum",
    "cksum",
    // grep family: `-f FILE` reads a pattern file; nothing writes or execs
    "grep",
    "egrep",
    "fgrep", // diff writes only to stdout (`sdiff --diff-program` is *not* here)
    "diff",  // jq's filter language has no shell-out and no output-file option
    "jq",
];

type Entry = (&'static str, ArgPolicy);

static TABLE: &[Entry] = &[
    (
        "git",
        ArgPolicy::Subcommand {
            subs: GIT_SUBCOMMANDS,
            opts: GIT_OPTS,
        },
    ),
    ("rg", ArgPolicy::Getopt(RG_OPTS)),
    ("bat", ArgPolicy::Getopt(BAT_OPTS)),
    ("sort", ArgPolicy::Getopt(SORT_OPTS)),
    ("tree", ArgPolicy::Getopt(TREE_OPTS)),
    ("date", ArgPolicy::Getopt(DATE_OPTS)),
    ("find", ArgPolicy::Primaries(FIND_PRIMARIES)),
    ("env", ArgPolicy::Assignments),
    ("cargo", ArgPolicy::Probe { extra: &["--list"] }),
    ("rustc", ArgPolicy::Probe { extra: &[] }),
    ("rustup", ArgPolicy::Probe { extra: &[] }),
    ("node", ArgPolicy::Probe { extra: &[] }),
    ("npm", ArgPolicy::Probe { extra: &[] }),
    ("python", ArgPolicy::Probe { extra: &[] }),
    ("python3", ArgPolicy::Probe { extra: &[] }),
    ("go", ArgPolicy::Probe { extra: &[] }),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_belongs_to_exactly_one_policy() {
        for (name, _) in TABLE {
            assert!(!INERT.contains(name), "{name} is listed twice");
        }
        let mut seen: Vec<&str> = TABLE.iter().map(|(n, _)| *n).collect();
        seen.sort_unstable();
        let len = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), len, "duplicate table entry");
    }

    #[test]
    fn unlisted_commands_have_no_policy() {
        assert!(policy_for("curl").is_none());
        assert!(policy_for("xargs").is_none());
        assert!(policy_for("sdiff").is_none());
        assert!(policy_for("rg").is_some());
    }
}
