//! Compaction planning: microcompaction clears old tool output in place;
//! summary compaction replaces older history with one summary message.
//! The engine performs the I/O (artifact spills, the summarizer request).

mod micro;
mod summary;
mod transcript;

pub use micro::{CLEARED_PREFIX, ClearTarget, apply_microcompact, plan_microcompact};
pub use summary::{SummaryPlan, apply_summary, plan_summary, summary_message};
pub use transcript::render_for_summary;
