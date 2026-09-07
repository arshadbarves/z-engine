//! What a run cost, and the running tally it is taken from.
//!
//! Metrics are appended once per turn rather than once per run, so a run
//! that is killed mid-flight still leaves the account it earned. Two
//! runs of the same work must agree on every field but the clock, which
//! is why the comparable projection leaves `wall_time_ms` out.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// What the run cost and how it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMetrics {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub turns: u64,
    /// Every tool call the model made, whatever the harness did with it
    /// — one per [`super::ToolOutcome`] on the tape. Counting only the
    /// calls that ran would make a run that was refused everything look
    /// like a run that asked for nothing.
    pub tool_calls: u64,
    pub wall_time_ms: u64,
    pub outcome: String,
}

/// The part of [`RunMetrics`] two runs of the same work must agree on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsFingerprint {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub turns: u64,
    pub tool_calls: u64,
    pub outcome: String,
}

impl RunMetrics {
    /// Everything except the clock.
    pub fn deterministic(&self) -> MetricsFingerprint {
        MetricsFingerprint {
            model_id: self.model_id.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            turns: self.turns,
            tool_calls: self.tool_calls,
            outcome: self.outcome.clone(),
        }
    }
}

/// The recorder's running tally, folded into a [`RunMetrics`] at the end
/// of every turn.
#[derive(Default, Debug)]
pub(super) struct Tally {
    model_id: String,
    input_tokens: u64,
    output_tokens: u64,
    turns: u64,
    tool_calls: u64,
    outcome: String,
}

/// Thread-safe wrapper: every fact arrives from whichever task produced
/// it, so the tally is shared rather than owned by the turn.
#[derive(Default, Debug)]
pub(super) struct SharedTally(Mutex<Tally>);

impl SharedTally {
    /// Fold one exchange's usage in, and adopt its model on the first
    /// request that names one.
    pub(super) fn add_usage(&self, model: &str, prompt_tokens: u64, completion_tokens: u64) {
        let mut tally = self.lock();
        if tally.model_id.is_empty() {
            tally.model_id = model.to_string();
        }
        tally.input_tokens += prompt_tokens;
        tally.output_tokens += completion_tokens;
    }

    /// Count one tool call the model made, whatever became of it.
    pub(super) fn add_tool_call(&self) {
        self.lock().tool_calls += 1;
    }

    /// Close a turn out and take the snapshot that goes on the tape.
    pub(super) fn close_turn(&self, outcome: &str, wall_time_ms: u64) -> RunMetrics {
        let mut tally = self.lock();
        tally.turns += 1;
        tally.outcome = outcome.to_string();
        RunMetrics {
            model_id: tally.model_id.clone(),
            input_tokens: tally.input_tokens,
            output_tokens: tally.output_tokens,
            turns: tally.turns,
            tool_calls: tally.tool_calls,
            wall_time_ms,
            outcome: tally.outcome.clone(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Tally> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clock is the one thing two runs of the same work may disagree
    /// about, so it is the one thing the comparison leaves out.
    #[test]
    fn metrics_compare_on_everything_but_the_clock() {
        let metrics = RunMetrics {
            model_id: "m".into(),
            input_tokens: 100,
            output_tokens: 20,
            turns: 1,
            tool_calls: 3,
            wall_time_ms: 42,
            outcome: "completed".into(),
        };
        let mut slower = metrics.clone();
        slower.wall_time_ms = 9_999;
        assert_eq!(slower.deterministic(), metrics.deterministic());

        let mut pricier = metrics.clone();
        pricier.output_tokens = 21;
        assert_ne!(pricier.deterministic(), metrics.deterministic());
    }

    #[test]
    fn the_tally_keeps_the_first_model_and_sums_the_rest() {
        let tally = SharedTally::default();
        tally.add_usage("first-model", 10, 5);
        tally.add_usage("second-model", 20, 5);
        tally.add_tool_call();
        tally.add_tool_call();

        let metrics = tally.close_turn("completed", 7);
        assert_eq!(metrics.model_id, "first-model");
        assert_eq!((metrics.input_tokens, metrics.output_tokens), (30, 10));
        assert_eq!((metrics.turns, metrics.tool_calls), (1, 2));
        assert_eq!(metrics.wall_time_ms, 7);
        assert_eq!(tally.close_turn("aborted", 9).turns, 2);
    }
}
