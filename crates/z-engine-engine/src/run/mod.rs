//! Agent runs: the round loop shared by the main agent and subagents,
//! with request assembly, streaming, context pressure, compaction, the
//! stop boundary, reminders and usage accounting.

mod agent;
mod budget;
mod compact;
mod meter;
mod pressure;
mod reminders;
mod request;
mod side;
mod sink;
mod spec;
mod stop;
mod stream;
mod usage;

pub(crate) use agent::AgentRun;
pub(crate) use compact::{CompactJob, Trigger, summarize};
pub(crate) use meter::ContextMeter;
pub(crate) use reminders::collect as collect_reminders;
pub(crate) use request::prepare as prepare_request;
pub(crate) use side::side_request;
pub(crate) use sink::{MainSink, TranscriptSink};
pub(crate) use spec::{AgentSpec, RunContext, RunOutcome};
