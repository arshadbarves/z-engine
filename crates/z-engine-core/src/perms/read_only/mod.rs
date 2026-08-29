//! The read-only *proof*: whether one shell segment can be shown, before
//! it runs, to execute no program and write no file.
//!
//! Two properties make this a proof rather than a habit:
//!
//! 1. **The command must be on the table** (`table`). Nothing else is
//!    considered, ever.
//! 2. **Its options are default-deny** (`scan`). Only option forms whose
//!    no-exec/no-write semantics were reviewed are accepted; an option
//!    nobody has ruled on is unproven, and unproven is refused.
//!
//! Rule 2 is what keeps the answer honest as tools grow: `rg --pre CMD`,
//! `bat --pager CMD`, `sort --compress-program CMD` and `tree -o FILE`
//! are all "read-only commands" running arbitrary programs or writing
//! files, and each was previously admitted by a command-name allowlist.
//! Under default-deny they are refused because nobody proved them safe —
//! and so is the next such option, without anyone having to hear about it
//! first.
//!
//! Refusing here is never a hard stop: it routes the call to the approval
//! prompt in ordinary runs, and to a typed gate failure in guarded ones.

mod options;
mod proof;
mod scan;
mod table;

pub(in crate::perms) use proof::segment_is_safe;
