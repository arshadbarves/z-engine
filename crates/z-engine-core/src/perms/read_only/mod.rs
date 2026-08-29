//! The read-only *proof*: whether one shell segment can be shown, before
//! it runs, to execute no program and write no file.
//!
//! Three properties make this a proof rather than a habit:
//!
//! 1. **The command must be on the table** (`table`). Nothing else is
//!    considered, ever.
//! 2. **Its options are default-deny** (`scan`). Only option forms whose
//!    no-exec/no-write semantics were reviewed are accepted; an option
//!    nobody has ruled on is unproven, and unproven is refused.
//! 3. **Its operands are classified** (`operands`). Every entry states
//!    what its operands *are*, because an inert option surface proves
//!    nothing about `uniq INPUT OUTPUT`.
//!
//! Rule 2 keeps the answer honest as tools grow: `rg --pre CMD`, `bat
//! --pager CMD`, `sort --compress-program CMD` and `tree -o FILE` are
//! all "read-only commands" running arbitrary programs or writing files,
//! and each was once admitted by a command-name allowlist.
//!
//! Rule 3 closes the same hole one level down. `uniq in out` writes
//! `out`, `hostname NAME` renames the machine and `date MMDDhhmm` sets
//! its clock, none of them with an option in sight; the table cannot say
//! "this command is safe" without also saying which of its operands it
//! merely reads.
//!
//! Under both rules the refusals extend to forms nobody has heard of
//! yet, which is the point: the next `--pre`-shaped flag, and the next
//! `[INPUT [OUTPUT]]`-shaped synopsis, are refused before anyone names
//! them.
//!
//! Refusing here is never a hard stop: it routes the call to the approval
//! prompt in ordinary runs, and to a typed gate failure in guarded ones.

mod operands;
mod options;
mod proof;
mod scan;
mod table;

pub(in crate::perms) use proof::segment_is_safe;
