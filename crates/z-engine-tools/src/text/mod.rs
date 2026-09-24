//! Text shaping shared by the tools: truncation with spill, line
//! numbering, and diffs.

mod diff;
mod numbering;
mod truncate;

pub(crate) use diff::{cap_preview, creation_diff, diff_stats, text_diff, unified_diff};
pub(crate) use numbering::{numbered, push_numbered};
pub(crate) use truncate::{truncate_output, truncate_parts, truncate_with};
