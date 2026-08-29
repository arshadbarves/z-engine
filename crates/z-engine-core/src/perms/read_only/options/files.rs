//! Readers over files: sort, tree, find, uniq and file.
//!
//! Every one of these has at least one documented way to be told to
//! write — an output option, an action primary, or an operand slot — so
//! none of them can be trusted by name.

use super::super::scan::Getopt;

/// sort, minus `-o`/`--output` (writes) and `--compress-program`,
/// `--random-source`, `--temporary-directory` (run or create files).
pub(in crate::perms::read_only) const SORT_OPTS: Getopt = Getopt {
    flags: "bdfghiMnrRsuVzcC",
    valued: "kt",
    longs: &[
        "ignore-leading-blanks",
        "dictionary-order",
        "ignore-case",
        "general-numeric-sort",
        "ignore-nonprinting",
        "month-sort",
        "human-numeric-sort",
        "numeric-sort",
        "reverse",
        "random-sort",
        "sort",
        "stable",
        "unique",
        "version-sort",
        "zero-terminated",
        "check",
        "merge",
        "parallel",
        "field-separator",
        "key",
        "debug",
    ],
    valued_longs: &[],
    digits: false,
};

/// tree, minus `-o` (writes its listing to a file).
pub(in crate::perms::read_only) const TREE_OPTS: Getopt = Getopt {
    flags: "adfghlnpqrstuvxACDFJQSUX",
    valued: "LPI",
    longs: &[
        "dirsfirst",
        "filelimit",
        "noreport",
        "charset",
        "du",
        "timefmt",
        "prune",
        "inodes",
        "device",
        "sort",
        "gitignore",
        "level",
        "json",
        "version",
        "help",
    ],
    valued_longs: &[],
    digits: false,
};

/// find, minus every action that runs a command (`-exec`, `-execdir`,
/// `-ok`, `-okdir`), deletes (`-delete`) or writes a file (`-fprint`,
/// `-fprint0`, `-fprintf`, `-fls`).
pub(in crate::perms::read_only) const FIND_PRIMARIES: &[&str] = &[
    "-H",
    "-L",
    "-P",
    "-maxdepth",
    "-mindepth",
    "-depth",
    "-mount",
    "-xdev",
    "-follow",
    "-name",
    "-iname",
    "-path",
    "-ipath",
    "-wholename",
    "-iwholename",
    "-regex",
    "-iregex",
    "-regextype",
    "-lname",
    "-ilname",
    "-type",
    "-xtype",
    "-size",
    "-empty",
    "-newer",
    "-newermt",
    "-anewer",
    "-cnewer",
    "-mtime",
    "-mmin",
    "-ctime",
    "-cmin",
    "-atime",
    "-amin",
    "-user",
    "-group",
    "-uid",
    "-gid",
    "-nouser",
    "-nogroup",
    "-perm",
    "-inum",
    "-links",
    "-samefile",
    "-readable",
    "-writable",
    "-executable",
    "-true",
    "-false",
    "-not",
    "-a",
    "-and",
    "-o",
    "-or",
    "-prune",
    "-quit",
    "-print",
    "-print0",
    "-printf",
    "-ls",
];

/// uniq. Its options are inert, but its synopsis is `uniq [OPTION]...
/// [INPUT [OUTPUT]]` — the second operand is written — so it is listed
/// here rather than trusted, and paired with an operand limit in the
/// table. `-f`/`-s`/`-w` and their long spellings take values, which the
/// scan must attribute so an operand count stays honest.
pub(in crate::perms::read_only) const UNIQ_OPTS: Getopt = Getopt {
    flags: "cdDuiz",
    valued: "fsw",
    longs: &[
        "count",
        "repeated",
        "all-repeated",
        "skip-fields",
        "skip-chars",
        "ignore-case",
        "unique",
        "check-chars",
        "zero-terminated",
        "group",
    ],
    valued_longs: &["skip-fields", "skip-chars", "check-chars"],
    digits: false,
};

/// file, minus `-C`/`--compile` (writes `<magic>.mgc`), `-z`/`-Z`
/// (may fork a decompressor), `-p`/`--preserve-date` (restores access
/// times) and every valued option (`-m`, `-M`, `-e`, `-F`, `-f`), which
/// are omitted rather than reviewed: refusing them costs a prompt.
pub(in crate::perms::read_only) const FILE_OPTS: Getopt = Getopt {
    flags: "bhikLNnrsv0",
    valued: "",
    longs: &[
        "brief",
        "mime",
        "mime-type",
        "mime-encoding",
        "dereference",
        "no-dereference",
        "keep-going",
        "no-buffer",
        "no-pad",
        "raw",
        "special-files",
        "extension",
        "print0",
        "version",
        "help",
    ],
    valued_longs: &[],
    digits: false,
};
