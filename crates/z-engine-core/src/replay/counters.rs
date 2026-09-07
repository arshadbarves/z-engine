//! Positions on the tape, counted per kind.
//!
//! "The third request" and "the third tool call" each mean what they
//! say, so a mismatch can be reported as a request number rather than a
//! line number. Exchanges count per lane: two callers sharing one
//! provider have no order between them, only an order within each.

use std::collections::HashMap;
use std::sync::Mutex;

use z_engine_provider::RequestLane;

#[derive(Default)]
struct Counts {
    exchanges: HashMap<RequestLane, u64>,
    prompts: u64,
    tools: u64,
    gates: u64,
    evidence: u64,
    completions: u64,
}

/// The sequence allocator for one cassette.
#[derive(Default)]
pub(super) struct Sequences {
    inner: Mutex<Counts>,
}

impl Sequences {
    /// The next request position *on `lane`*.
    pub(super) fn exchange(&self, lane: &RequestLane) -> u64 {
        let mut counts = self.lock();
        let slot = counts.exchanges.entry(lane.clone()).or_insert(0);
        let seq = *slot;
        *slot += 1;
        seq
    }

    pub(super) fn prompt(&self) -> u64 {
        self.next(|c| &mut c.prompts)
    }

    pub(super) fn tool(&self) -> u64 {
        self.next(|c| &mut c.tools)
    }

    pub(super) fn gate(&self) -> u64 {
        self.next(|c| &mut c.gates)
    }

    pub(super) fn evidence(&self) -> u64 {
        self.next(|c| &mut c.evidence)
    }

    pub(super) fn completion(&self) -> u64 {
        self.next(|c| &mut c.completions)
    }

    fn next(&self, field: impl Fn(&mut Counts) -> &mut u64) -> u64 {
        let mut counts = self.lock();
        let slot = field(&mut counts);
        let seq = *slot;
        *slot += 1;
        seq
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Counts> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_counts_from_zero_independently() {
        let seqs = Sequences::default();
        assert_eq!(seqs.prompt(), 0);
        assert_eq!(seqs.prompt(), 1);
        assert_eq!(seqs.tool(), 0);
        assert_eq!(seqs.gate(), 0);
        assert_eq!(seqs.evidence(), 0);
        assert_eq!(seqs.completion(), 0);
    }

    #[test]
    fn a_lane_counts_its_own_requests_and_no_one_elses() {
        let seqs = Sequences::default();
        let side = RequestLane::named("title");
        assert_eq!(seqs.exchange(&RequestLane::MAIN), 0);
        assert_eq!(seqs.exchange(&side), 0);
        assert_eq!(seqs.exchange(&RequestLane::MAIN), 1);
        assert_eq!(seqs.exchange(&side), 1);
    }
}
