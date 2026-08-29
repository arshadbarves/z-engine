//! How a command's arguments are read, and the one question asked of
//! them: is every argument inert — neither naming a program to run nor a
//! file to write?
//!
//! Default-deny throughout. An option form that is not explicitly listed
//! for its command is unproven, and unproven answers `false`.

/// How one command's arguments must be interpreted.
pub(super) enum ArgPolicy {
    /// Every argument is inert data because the command has *no* option
    /// that can execute a program or write a file. Reserved for commands
    /// whose whole documented option surface was reviewed (`cat`, `grep`,
    /// `wc`, …); it is not a shrug.
    Inert,
    /// GNU-style options, default-deny: `-ab`, `-n5`, `-n 5`, `--long`,
    /// `--long=value`, `--` ends option parsing.
    Getopt(Getopt),
    /// find(1): single-dash keyword primaries, no bundling. Everything
    /// unlisted (`-exec`, `-delete`, `-fprint`, `-ok`, …) is refused.
    Primaries(&'static [&'static str]),
    /// env(1): only `NAME=VALUE` assignments. Any other argument is the
    /// command env would run, which launders anything.
    Assignments,
    /// A subcommand allowlist first (git), then default-deny options.
    Subcommand {
        subs: &'static [&'static str],
        opts: Getopt,
    },
    /// Version probes: `<cmd> --version` and the listed equivalents only.
    Probe { extra: &'static [&'static str] },
}

/// The proven option surface of one command.
pub(super) struct Getopt {
    /// Short options taking no value; bundling allowed (`-la`).
    pub flags: &'static str,
    /// Short options that consume a value — the rest of the bundle
    /// (`-n5`) or the next word (`-n 5`). The value is data for a proven
    /// option, so it needs no further scrutiny.
    pub valued: &'static str,
    /// Long options, spelled in full, without `--`. Accepted bare or as
    /// `--name=value`. Abbreviations are refused: `--out` may abbreviate
    /// a *denied* option, so only exact names prove.
    pub longs: &'static [&'static str],
    /// Whether bare digits are operands rather than options (`head -5`,
    /// `git log -10`).
    pub digits: bool,
}

impl ArgPolicy {
    /// True when this command has no options at all worth constraining —
    /// the only case where an unquoted glob is harmless, since expansion
    /// cannot conjure a dangerous option.
    pub(super) fn is_option_free(&self) -> bool {
        matches!(self, Self::Inert)
    }

    /// The whole question: may these arguments run under this policy?
    pub(super) fn admits(&self, args: &[String]) -> bool {
        match self {
            Self::Inert => true,
            Self::Getopt(opts) => getopt_admits(opts, args),
            Self::Primaries(allowed) => args.iter().all(|a| primary_admits(allowed, a)),
            Self::Assignments => args.iter().all(|a| !a.starts_with('-') && a.contains('=')),
            Self::Subcommand { subs, opts } => match args.split_first() {
                Some((sub, rest)) => subs.contains(&sub.as_str()) && getopt_admits(opts, rest),
                None => false,
            },
            Self::Probe { extra } => {
                !args.is_empty()
                    && args
                        .iter()
                        .all(|a| a == "--version" || a == "-V" || extra.contains(&a.as_str()))
            }
        }
    }
}

fn getopt_admits(opts: &Getopt, args: &[String]) -> bool {
    let mut operands_only = false;
    for arg in args {
        if operands_only {
            continue;
        }
        if arg == "--" {
            operands_only = true;
            continue;
        }
        if let Some(long) = arg.strip_prefix("--") {
            let name = long.split_once('=').map_or(long, |(name, _)| name);
            if !opts.longs.contains(&name) {
                return false;
            }
            continue;
        }
        match arg.strip_prefix('-') {
            // A bare `-` is the stdin operand, not an option.
            Some("") | None => continue,
            Some(bundle) => {
                if !bundle_admits(opts, bundle) {
                    return false;
                }
            }
        }
    }
    true
}

/// Scan one short-option bundle left to right. A valued option swallows
/// whatever follows it in the bundle, so `-k2` proves and `-ofile` does
/// not (unless `o` itself was proven).
fn bundle_admits(opts: &Getopt, bundle: &str) -> bool {
    for c in bundle.chars() {
        if opts.valued.contains(c) {
            return true;
        }
        if opts.flags.contains(c) || (opts.digits && c.is_ascii_digit()) {
            continue;
        }
        return false;
    }
    true
}

/// find(1) arguments: operands and `-<number>` bounds pass; keyword
/// primaries must be on the list.
fn primary_admits(allowed: &[&str], arg: &str) -> bool {
    let Some(primary) = arg.strip_prefix('-') else {
        return true;
    };
    if !primary.is_empty() && primary.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    allowed.contains(&arg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    const OPTS: Getopt = Getopt {
        flags: "ab",
        valued: "k",
        longs: &["keep", "width"],
        digits: false,
    };

    #[test]
    fn getopt_accepts_proven_forms_and_refuses_everything_else() {
        let policy = ArgPolicy::Getopt(OPTS);
        assert!(policy.admits(&args(&["-a", "-b", "file"])));
        assert!(policy.admits(&args(&["-ab"])), "bundled");
        assert!(policy.admits(&args(&["-k2"])), "attached value");
        assert!(policy.admits(&args(&["-k", "2"])), "separated value");
        assert!(policy.admits(&args(&["--keep", "--width=3"])));
        assert!(policy.admits(&args(&["--", "-anything"])), "after --");
        assert!(!policy.admits(&args(&["-c"])), "unlisted short");
        assert!(!policy.admits(&args(&["-ac"])), "unlisted inside bundle");
        assert!(!policy.admits(&args(&["--other"])), "unlisted long");
        assert!(
            !policy.admits(&args(&["--kee"])),
            "abbreviations never prove"
        );
    }

    #[test]
    fn digits_are_operands_only_where_the_command_says_so() {
        assert!(!ArgPolicy::Getopt(OPTS).admits(&args(&["-5"])));
        let numeric = ArgPolicy::Getopt(Getopt {
            digits: true,
            ..OPTS
        });
        assert!(numeric.admits(&args(&["-5"])));
    }

    #[test]
    fn find_primaries_allow_operands_and_numeric_bounds() {
        let policy = ArgPolicy::Primaries(&["-name", "-mtime"]);
        assert!(policy.admits(&args(&[".", "-name", "*.rs"])));
        assert!(policy.admits(&args(&["-mtime", "-1"])), "numeric bound");
        assert!(!policy.admits(&args(&["-delete"])));
    }

    #[test]
    fn assignments_refuse_a_command_operand() {
        let policy = ArgPolicy::Assignments;
        assert!(policy.admits(&args(&[])));
        assert!(policy.admits(&args(&["FOO=1"])));
        assert!(!policy.admits(&args(&["FOO=1", "rm"])));
        assert!(!policy.admits(&args(&["-i"])));
    }

    #[test]
    fn subcommands_and_probes_are_closed_sets() {
        let policy = ArgPolicy::Subcommand {
            subs: &["status"],
            opts: OPTS,
        };
        assert!(policy.admits(&args(&["status", "-a"])));
        assert!(!policy.admits(&args(&["push"])));
        assert!(!policy.admits(&args(&[])), "bare command proves nothing");
        assert!(!policy.admits(&args(&["-a", "status"])), "options first");

        let probe = ArgPolicy::Probe { extra: &["--list"] };
        assert!(probe.admits(&args(&["--version"])));
        assert!(probe.admits(&args(&["--list"])));
        assert!(!probe.admits(&args(&[])));
        assert!(!probe.admits(&args(&["test"])));
    }
}
