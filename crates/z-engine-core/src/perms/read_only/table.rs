//! The table of commands that may appear in a read-only proof. Data
//! only — `scan` and `operands` interpret it, `proof` applies it.
//!
//! Every entry makes **two** claims, and both must be written out:
//!
//! 1. an [`ArgPolicy`] — what its options can be told to do, and
//! 2. an [`Operands`] rule — what its operands *are*.
//!
//! The second exists because the first is not enough. `uniq`, `hostname`
//! and `date` all have inert option surfaces and all write a file or
//! change the machine through a bare operand, so a table that only
//! ruled on options was admitting `uniq INPUT OUTPUT`. There is no
//! default for either claim: adding a command means answering both
//! questions in the same line a reviewer reads.

use super::operands::Operands;
use super::options::{
    BAT_OPTS, DATE_OPTS, FILE_OPTS, FIND_PRIMARIES, GIT_OPTS, GIT_SUBCOMMANDS, HOSTNAME_OPTS,
    RG_OPTS, SORT_OPTS, TREE_OPTS, UNIQ_OPTS,
};
use super::scan::ArgPolicy;

/// The policy and operand rule for `command`, or `None` when it may
/// never appear in a proof.
pub(super) fn entry_for(command: &str) -> Option<(&'static ArgPolicy, &'static Operands)> {
    TABLE
        .iter()
        .find(|(name, ..)| *name == command)
        .map(|(_, policy, operands)| (policy, operands))
}

/// No option of this command can execute a program or write a file: its
/// whole documented option surface was reviewed.
const INERT_OPTIONS: ArgPolicy = ArgPolicy::InertOptions;
/// Every operand is something the command reads.
const ALL_INPUTS: Operands = Operands::AllInputs;

type Entry = (&'static str, ArgPolicy, Operands);

static TABLE: &[Entry] = &[
    // coreutils readers: every option is a formatting or selection knob,
    // and every operand is a file, name or number they only read.
    ("ls", INERT_OPTIONS, ALL_INPUTS),
    ("cat", INERT_OPTIONS, ALL_INPUTS),
    ("head", INERT_OPTIONS, ALL_INPUTS),
    ("tail", INERT_OPTIONS, ALL_INPUTS),
    ("wc", INERT_OPTIONS, ALL_INPUTS),
    ("nl", INERT_OPTIONS, ALL_INPUTS),
    ("fmt", INERT_OPTIONS, ALL_INPUTS),
    ("pr", INERT_OPTIONS, ALL_INPUTS),
    ("numfmt", INERT_OPTIONS, ALL_INPUTS),
    ("tsort", INERT_OPTIONS, ALL_INPUTS),
    ("comm", INERT_OPTIONS, ALL_INPUTS),
    ("cmp", INERT_OPTIONS, ALL_INPUTS),
    ("seq", INERT_OPTIONS, ALL_INPUTS),
    ("expr", INERT_OPTIONS, ALL_INPUTS),
    ("test", INERT_OPTIONS, ALL_INPUTS),
    ("[", INERT_OPTIONS, ALL_INPUTS),
    ("basename", INERT_OPTIONS, ALL_INPUTS),
    ("dirname", INERT_OPTIONS, ALL_INPUTS),
    ("realpath", INERT_OPTIONS, ALL_INPUTS),
    ("readlink", INERT_OPTIONS, ALL_INPUTS),
    ("printenv", INERT_OPTIONS, ALL_INPUTS),
    ("id", INERT_OPTIONS, ALL_INPUTS),
    ("whoami", INERT_OPTIONS, ALL_INPUTS),
    ("uname", INERT_OPTIONS, ALL_INPUTS),
    ("getconf", INERT_OPTIONS, ALL_INPUTS),
    ("pwd", INERT_OPTIONS, ALL_INPUTS),
    ("echo", INERT_OPTIONS, ALL_INPUTS),
    ("printf", INERT_OPTIONS, ALL_INPUTS),
    ("cd", INERT_OPTIONS, ALL_INPUTS),
    ("which", INERT_OPTIONS, ALL_INPUTS),
    ("type", INERT_OPTIONS, ALL_INPUTS),
    ("stat", INERT_OPTIONS, ALL_INPUTS),
    ("du", INERT_OPTIONS, ALL_INPUTS),
    ("df", INERT_OPTIONS, ALL_INPUTS),
    // hashers: read and print, no output-file option, no output operand
    ("md5sum", INERT_OPTIONS, ALL_INPUTS),
    ("shasum", INERT_OPTIONS, ALL_INPUTS),
    ("sha256sum", INERT_OPTIONS, ALL_INPUTS),
    ("cksum", INERT_OPTIONS, ALL_INPUTS),
    // grep family: `-f FILE` reads a pattern file; nothing writes or execs
    ("grep", INERT_OPTIONS, ALL_INPUTS),
    ("egrep", INERT_OPTIONS, ALL_INPUTS),
    ("fgrep", INERT_OPTIONS, ALL_INPUTS),
    // diff writes only to stdout (`sdiff --diff-program` is *not* here)
    ("diff", INERT_OPTIONS, ALL_INPUTS),
    // jq's filter language has no shell-out and no output-file option
    ("jq", INERT_OPTIONS, ALL_INPUTS),
    // Commands whose options had to be listed, because at least one of
    // them runs a program or writes a file.
    (
        "git",
        ArgPolicy::Subcommand {
            subs: GIT_SUBCOMMANDS,
            opts: GIT_OPTS,
        },
        ALL_INPUTS,
    ),
    ("rg", ArgPolicy::Getopt(RG_OPTS), ALL_INPUTS),
    ("bat", ArgPolicy::Getopt(BAT_OPTS), ALL_INPUTS),
    ("sort", ArgPolicy::Getopt(SORT_OPTS), ALL_INPUTS),
    ("tree", ArgPolicy::Getopt(TREE_OPTS), ALL_INPUTS),
    ("find", ArgPolicy::Primaries(FIND_PRIMARIES), ALL_INPUTS),
    ("env", ArgPolicy::Assignments, ALL_INPUTS),
    // …and commands whose *operands* had to be limited, because the
    // synopsis gives a later one another meaning.
    //
    // `uniq [OPTION]... [INPUT [OUTPUT]]` — the second operand is written.
    (
        "uniq",
        ArgPolicy::Getopt(UNIQ_OPTS),
        Operands::InputsAtMost(1),
    ),
    // `hostname [NAME]` — the only operand *sets* the system hostname.
    (
        "hostname",
        ArgPolicy::Getopt(HOSTNAME_OPTS),
        Operands::InputsAtMost(0),
    ),
    // `date [+FORMAT]` asks; `date MMDDhhmm[[CC]YY][.ss]` sets the clock.
    (
        "date",
        ArgPolicy::Getopt(DATE_OPTS),
        Operands::OnlyPrefixed("+"),
    ),
    // file(1) reads its operands, but `-C` compiles magic to a file.
    ("file", ArgPolicy::Getopt(FILE_OPTS), ALL_INPUTS),
    ("cargo", ArgPolicy::Probe { extra: &["--list"] }, ALL_INPUTS),
    ("rustc", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
    ("rustup", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
    ("node", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
    ("npm", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
    ("python", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
    ("python3", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
    ("go", ArgPolicy::Probe { extra: &[] }, ALL_INPUTS),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_appears_at_most_once() {
        let mut seen: Vec<&str> = TABLE.iter().map(|(n, ..)| *n).collect();
        seen.sort_unstable();
        let len = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), len, "duplicate table entry");
    }

    #[test]
    fn unlisted_commands_have_no_entry() {
        assert!(entry_for("curl").is_none());
        assert!(entry_for("xargs").is_none());
        assert!(entry_for("sdiff").is_none());
        assert!(entry_for("rg").is_some());
    }

    /// Counting operands only works where option values can be told
    /// apart from operands, which needs an explicit option model.
    /// `InertOptions` reports a conservative superset, so pairing it with
    /// a limit would refuse ordinary work for the wrong reason — and
    /// worse, would read like a proof. Every limited entry must carry a
    /// real option surface.
    #[test]
    fn an_operand_limit_requires_an_option_model() {
        for (name, policy, operands) in TABLE {
            if operands.expansion_only_adds_inputs() {
                continue;
            }
            assert!(
                !policy.options_are_inert(),
                "{name} limits its operands, so its options must be modelled"
            );
        }
    }

    /// The three commands this rule exists for. If someone relaxes them
    /// back to "all operands are inputs", this fails rather than the
    /// predicate quietly starting to admit `uniq in out` again.
    #[test]
    fn commands_with_an_output_or_setting_operand_stay_limited() {
        for name in ["uniq", "hostname", "date"] {
            let (_, operands) = entry_for(name).expect("listed");
            assert!(
                !operands.expansion_only_adds_inputs(),
                "{name} has an operand that is not an input"
            );
        }
    }
}
