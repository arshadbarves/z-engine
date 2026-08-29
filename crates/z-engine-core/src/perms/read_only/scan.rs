//! How a command's arguments are read: which words are options, which
//! are those options' values, and — what is left over — which are
//! operands.
//!
//! Two questions are asked here, and both must be answered before a
//! segment can be called read-only:
//!
//! 1. Is every option form proven not to execute a program or write a
//!    file? Default-deny: an option form nobody has ruled on is unproven.
//! 2. Which words are the *operands*? [`super::operands`] rules on those
//!    separately, because a perfectly inert option surface says nothing
//!    about `uniq INPUT OUTPUT`.
//!
//! Attribution is deliberately conservative: where a policy cannot tell
//! an option's value from an operand, the value is reported as an
//! operand. That can only over-count, which can only refuse.

/// How one command's arguments must be interpreted.
#[derive(Clone, Copy)]
pub(super) enum ArgPolicy {
    /// The command has *no* option that can execute a program or write a
    /// file, so no option form needs listing. Reserved for commands whose
    /// whole documented option surface was reviewed (`cat`, `grep`,
    /// `wc`, …); it is not a shrug, and it says nothing about operands.
    InertOptions,
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
#[derive(Clone, Copy)]
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
    /// Long options that take their value as the *next* word when
    /// spelled without `=`. Load-bearing only where the operand rule
    /// limits arity — elsewhere an unattributed value is counted as one
    /// more input, which changes no answer.
    pub valued_longs: &'static [&'static str],
    /// Whether bare digits are operands rather than options (`head -5`,
    /// `git log -10`).
    pub digits: bool,
}

impl ArgPolicy {
    /// True when this command has no options at all worth constraining.
    pub(super) fn options_are_inert(&self) -> bool {
        matches!(self, Self::InertOptions)
    }

    /// The operands these arguments leave behind, or `None` when some
    /// option form was not proven — in which case there is nothing left
    /// to rule on, because the command is already refused.
    pub(super) fn operands<'a>(&self, args: &'a [String]) -> Option<Vec<&'a str>> {
        match self {
            Self::InertOptions => Some(bare_words(args)),
            Self::Getopt(opts) => getopt_operands(opts, args),
            Self::Primaries(allowed) => args
                .iter()
                .all(|a| primary_admits(allowed, a))
                .then(|| bare_words(args)),
            Self::Assignments => args
                .iter()
                .all(|a| !a.starts_with('-') && a.contains('='))
                .then(Vec::new),
            Self::Subcommand { subs, opts } => match args.split_first() {
                Some((sub, rest)) if subs.contains(&sub.as_str()) => getopt_operands(opts, rest),
                _ => None,
            },
            Self::Probe { extra } => (!args.is_empty()
                && args
                    .iter()
                    .all(|a| a == "--version" || a == "-V" || extra.contains(&a.as_str())))
            .then(Vec::new),
        }
    }
}

/// Every word that is not an option word. A value that follows a valued
/// option is counted here too, because a policy with no option model
/// cannot tell the two apart — over-counting can only refuse.
fn bare_words(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut operands_only = false;
    for arg in args {
        if operands_only {
            out.push(arg.as_str());
            continue;
        }
        if arg == "--" {
            operands_only = true;
            continue;
        }
        if arg.len() > 1 && arg.starts_with('-') {
            continue;
        }
        out.push(arg.as_str());
    }
    out
}

fn getopt_operands<'a>(opts: &Getopt, args: &'a [String]) -> Option<Vec<&'a str>> {
    let mut operands = Vec::new();
    let mut operands_only = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        if operands_only {
            operands.push(arg);
            continue;
        }
        if arg == "--" {
            operands_only = true;
            continue;
        }
        if let Some(long) = arg.strip_prefix("--") {
            let (name, attached) = match long.split_once('=') {
                Some((name, _)) => (name, true),
                None => (long, false),
            };
            if !opts.longs.contains(&name) {
                return None;
            }
            if !attached && opts.valued_longs.contains(&name) && consumes_next(args, i) {
                i += 1;
            }
            continue;
        }
        match arg.strip_prefix('-') {
            // A bare `-` is the stdin operand, not an option.
            Some("") | None => operands.push(arg),
            Some(bundle) => match scan_bundle(opts, bundle) {
                Bundle::Refused => return None,
                Bundle::Complete => {}
                Bundle::NeedsValue => {
                    if consumes_next(args, i) {
                        i += 1;
                    }
                }
            },
        }
    }
    Some(operands)
}

/// Whether a valued option takes the next word. A word starting with `-`
/// is not consumed: treating it as a value would hide it from the scan,
/// which is how `rg --file --pre CMD` would otherwise prove.
fn consumes_next(args: &[String], next: usize) -> bool {
    args.get(next).is_some_and(|v| !v.starts_with('-'))
}

enum Bundle {
    /// Every letter was a proven flag, or a valued option took its value
    /// from inside the bundle; no following word belongs to it.
    Complete,
    /// The bundle ended on a valued option, so the next word is its value.
    NeedsValue,
    Refused,
}

/// Scan one short-option bundle left to right. A valued option swallows
/// whatever follows it in the bundle, so `-k2` proves and `-ofile` does
/// not (unless `o` itself was proven).
fn scan_bundle(opts: &Getopt, bundle: &str) -> Bundle {
    let mut chars = bundle.chars();
    while let Some(c) = chars.next() {
        if opts.valued.contains(c) {
            return if chars.as_str().is_empty() {
                Bundle::NeedsValue
            } else {
                Bundle::Complete
            };
        }
        if opts.flags.contains(c) || (opts.digits && c.is_ascii_digit()) {
            continue;
        }
        return Bundle::Refused;
    }
    Bundle::Complete
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

    fn admits(policy: &ArgPolicy, list: &[&str]) -> bool {
        policy.operands(&args(list)).is_some()
    }

    fn operands(policy: &ArgPolicy, list: &[&str]) -> Vec<String> {
        policy
            .operands(&args(list))
            .expect("proven")
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    const OPTS: Getopt = Getopt {
        flags: "ab",
        valued: "k",
        longs: &["keep", "width"],
        valued_longs: &["width"],
        digits: false,
    };

    #[test]
    fn getopt_accepts_proven_forms_and_refuses_everything_else() {
        let policy = ArgPolicy::Getopt(OPTS);
        assert!(admits(&policy, &["-a", "-b", "file"]));
        assert!(admits(&policy, &["-ab"]), "bundled");
        assert!(admits(&policy, &["-k2"]), "attached value");
        assert!(admits(&policy, &["-k", "2"]), "separated value");
        assert!(admits(&policy, &["--keep", "--width=3"]));
        assert!(admits(&policy, &["--", "-anything"]), "after --");
        assert!(!admits(&policy, &["-c"]), "unlisted short");
        assert!(!admits(&policy, &["-ac"]), "unlisted inside bundle");
        assert!(!admits(&policy, &["--other"]), "unlisted long");
        assert!(!admits(&policy, &["--kee"]), "abbreviations never prove");
    }

    /// Operand attribution is the second half of the answer, so a value
    /// must not be mistaken for an operand — nor an operand for a value.
    #[test]
    fn option_values_are_not_operands() {
        let policy = ArgPolicy::Getopt(OPTS);
        assert_eq!(operands(&policy, &["-k", "2", "in"]), ["in"]);
        assert_eq!(operands(&policy, &["-k2", "in"]), ["in"]);
        assert_eq!(operands(&policy, &["--width", "3", "in"]), ["in"]);
        assert_eq!(operands(&policy, &["--width=3", "in"]), ["in"]);
        assert_eq!(
            operands(&policy, &["--keep", "in"]),
            ["in"],
            "a valueless long consumes nothing"
        );
        assert_eq!(operands(&policy, &["--", "-a", "in"]), ["-a", "in"]);
        assert_eq!(operands(&policy, &["-"]), ["-"], "stdin is an operand");
    }

    /// A valued option must not swallow the next *option*: doing so
    /// would hide it from the scan, which is how `rg --file --pre CMD`
    /// would otherwise prove.
    #[test]
    fn a_value_never_hides_the_option_that_follows_it() {
        let policy = ArgPolicy::Getopt(OPTS);
        assert!(!admits(&policy, &["-k", "-c"]));
        assert!(!admits(&policy, &["--width", "--other"]));
    }

    /// Where a policy has no option model, values are counted as
    /// operands. That is the safe direction and must stay that way.
    #[test]
    fn policies_without_an_option_model_over_count_operands() {
        let policy = ArgPolicy::InertOptions;
        assert_eq!(operands(&policy, &["-n", "5", "file"]), ["5", "file"]);
        assert_eq!(operands(&policy, &["--", "-weird"]), ["-weird"]);
    }

    #[test]
    fn digits_are_operands_only_where_the_command_says_so() {
        assert!(!admits(&ArgPolicy::Getopt(OPTS), &["-5"]));
        let numeric = ArgPolicy::Getopt(Getopt {
            digits: true,
            ..OPTS
        });
        assert!(admits(&numeric, &["-5"]));
    }

    #[test]
    fn find_primaries_allow_operands_and_numeric_bounds() {
        let policy = ArgPolicy::Primaries(&["-name", "-mtime"]);
        assert!(admits(&policy, &[".", "-name", "*.rs"]));
        assert!(admits(&policy, &["-mtime", "-1"]), "numeric bound");
        assert!(!admits(&policy, &["-delete"]));
    }

    #[test]
    fn assignments_refuse_a_command_operand() {
        let policy = ArgPolicy::Assignments;
        assert!(admits(&policy, &[]));
        assert!(admits(&policy, &["FOO=1"]));
        assert!(!admits(&policy, &["FOO=1", "rm"]));
        assert!(!admits(&policy, &["-i"]));
    }

    #[test]
    fn subcommands_and_probes_are_closed_sets() {
        let policy = ArgPolicy::Subcommand {
            subs: &["status"],
            opts: OPTS,
        };
        assert!(admits(&policy, &["status", "-a"]));
        assert!(!admits(&policy, &["push"]));
        assert!(!admits(&policy, &[]), "bare command proves nothing");
        assert!(!admits(&policy, &["-a", "status"]), "options first");
        assert_eq!(
            operands(&policy, &["status", "src"]),
            ["src"],
            "the subcommand is not an operand"
        );

        let probe = ArgPolicy::Probe { extra: &["--list"] };
        assert!(admits(&probe, &["--version"]));
        assert!(admits(&probe, &["--list"]));
        assert!(!admits(&probe, &[]));
        assert!(!admits(&probe, &["test"]));
    }
}
