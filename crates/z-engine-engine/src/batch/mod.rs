//! The tool gate and executor: validation, hooks, policy, batched
//! approvals, ordered execution with parallel safe segments.

mod approval;
mod call;
mod ctx;
mod execute;
mod gate;
mod progress;
mod report;
mod schema;
mod toolset;

pub(crate) use execute::run_batch;
pub(crate) use gate::ToolCall;
pub(crate) use toolset::ToolSet;
