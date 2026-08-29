//! The audited option surfaces: for each command whose options are *not*
//! uniformly inert, the exact forms proven neither to execute a program
//! nor to write a file.
//!
//! Kept apart from `table` because these change for a different reason —
//! a command grows a new flag, or a manual is re-read — and because every
//! omission here is load-bearing: a form that is absent is refused, which
//! is how `rg --pre`, `bat --pager` and `sort --compress-program` stay
//! out of a proof without anyone having to enumerate the dangerous forms.
//!
//! Split by the manual a reviewer would have to open: version control,
//! search, files, machine.

mod files;
mod git;
mod search;
mod system;

pub(super) use files::{FILE_OPTS, FIND_PRIMARIES, SORT_OPTS, TREE_OPTS, UNIQ_OPTS};
pub(super) use git::{GIT_OPTS, GIT_SUBCOMMANDS};
pub(super) use search::{BAT_OPTS, RG_OPTS};
pub(super) use system::{DATE_OPTS, HOSTNAME_OPTS};
