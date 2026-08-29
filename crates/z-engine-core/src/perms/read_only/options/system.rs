//! Commands that report on the machine: date and hostname.
//!
//! Both can be asked to *change* the machine — the clock through an
//! operand, the hostname through an operand or `-F` — so both are listed
//! and both carry an operand rule in the table.

use super::super::scan::Getopt;

/// date, minus `-s`/`--set` (sets the system clock).
pub(in crate::perms::read_only) const DATE_OPTS: Getopt = Getopt {
    flags: "uR",
    valued: "dfr",
    longs: &[
        "utc",
        "universal",
        "rfc-3339",
        "rfc-email",
        "iso-8601",
        "date",
        "file",
        "reference",
        "debug",
    ],
    valued_longs: &["date", "file", "reference"],
    digits: false,
};

/// hostname, minus every form that *sets* a name: a bare operand,
/// `-b`/`--boot`, `-F`/`--file`, and the NIS setters. What is left only
/// reports, and the table pairs it with a zero-operand rule so the
/// setting form cannot come back as an argument.
pub(in crate::perms::read_only) const HOSTNAME_OPTS: Getopt = Getopt {
    flags: "sfdiIAa",
    valued: "",
    longs: &[
        "fqdn",
        "long",
        "short",
        "domain",
        "all-fqdns",
        "all-ip-addresses",
        "ip-address",
        "alias",
    ],
    valued_longs: &[],
    digits: false,
};
