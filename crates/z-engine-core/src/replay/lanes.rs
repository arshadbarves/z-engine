//! Lane bookkeeping for replay: the recorded requests of one run, split
//! by the caller that issued them, and checked for holes before any of
//! them is served.
//!
//! A run's requests do not form one sequence. The turn loop, the session
//! titler, the reviewer and each sub-agent all speak to the provider
//! independently, and nothing orders them against each other — so a
//! single cursor over the whole tape matches whichever caller happened
//! to win the race that day. Per-lane cursors match each caller against
//! its own requests, which *are* ordered, and leave the interleaving
//! unconstrained where it was never constrained to begin with.

use std::collections::HashMap;
use std::path::Path;

use z_engine_provider::RequestLane;

use super::entry::ProviderExchange;
use super::error::ReplayError;
use super::tape::RunCassette;

/// The recorded exchanges of one run, grouped by lane.
#[derive(Debug, Default)]
pub struct LaneTable {
    lanes: HashMap<RequestLane, Vec<ProviderExchange>>,
    total: usize,
}

impl LaneTable {
    /// Group a cassette's exchanges and refuse any lane with a hole in
    /// it.
    ///
    /// Sequences are allocated per lane from zero without gaps, so a
    /// missing or duplicated number means the tape is not the run: an
    /// entry was lost, or two tapes were spliced. Serving it anyway
    /// would silently shift every later request by one and report the
    /// divergence somewhere it did not happen.
    pub fn build(cassette: &RunCassette) -> Result<Self, ReplayError> {
        let mut lanes: HashMap<RequestLane, Vec<ProviderExchange>> = HashMap::new();
        let mut total = 0;
        for exchange in cassette.exchanges() {
            total += 1;
            lanes
                .entry(exchange.lane.clone())
                .or_default()
                .push(exchange);
        }
        for (lane, exchanges) in &lanes {
            check_contiguous(cassette.path(), lane, exchanges)?;
        }
        Ok(Self { lanes, total })
    }

    /// The recorded requests on one lane, in issue order.
    pub fn lane(&self, lane: &RequestLane) -> &[ProviderExchange] {
        self.lanes.get(lane).map_or(&[], Vec::as_slice)
    }

    /// How many requests the whole run made.
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Report the *first* place a lane stops counting 0, 1, 2, …
fn check_contiguous(
    path: &Path,
    lane: &RequestLane,
    exchanges: &[ProviderExchange],
) -> Result<(), ReplayError> {
    for (index, exchange) in exchanges.iter().enumerate() {
        let expected = index as u64;
        if exchange.sequence != expected {
            return Err(ReplayError::SequenceGap {
                path: path.to_path_buf(),
                lane: lane.label().to_string(),
                expected,
                found: exchange.sequence,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::entry::{CassetteEntry, canonical_request};
    use crate::replay::sink::{CassetteWriter, EntrySink};
    use z_engine_provider::{ChatMessage, ChatRequest, StreamEvent};

    fn taped(dir: &tempfile::TempDir, entries: &[(RequestLane, u64)]) -> RunCassette {
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        for (lane, sequence) in entries {
            let request = ChatRequest::new("m", vec![ChatMessage::user("hi")]);
            let (_, hash) = canonical_request(&request).unwrap();
            writer
                .append(&CassetteEntry::Exchange(ProviderExchange {
                    lane: lane.clone(),
                    sequence: *sequence,
                    request_hash: hash,
                    request,
                    events: vec![StreamEvent::Done],
                    error: None,
                }))
                .unwrap();
        }
        RunCassette::load(&path).unwrap()
    }

    #[test]
    fn lanes_are_grouped_and_counted_separately() {
        let dir = tempfile::tempdir().unwrap();
        let title = RequestLane::named("title");
        let cassette = taped(
            &dir,
            &[
                (RequestLane::MAIN, 0),
                (title.clone(), 0),
                (RequestLane::MAIN, 1),
            ],
        );
        let table = LaneTable::build(&cassette).unwrap();
        assert_eq!(table.lane(&RequestLane::MAIN).len(), 2);
        assert_eq!(table.lane(&title).len(), 1);
        assert_eq!(table.lane(&RequestLane::named("nobody")).len(), 0);
        assert_eq!(table.total(), 3);
    }

    /// A tape missing request #1 would otherwise serve #2 in its place
    /// and blame the divergence on the run.
    #[test]
    fn a_gap_in_a_lane_is_refused_with_the_first_missing_request() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(&dir, &[(RequestLane::MAIN, 0), (RequestLane::MAIN, 2)]);
        let err = LaneTable::build(&cassette).unwrap_err();
        assert!(
            matches!(
                &err,
                ReplayError::SequenceGap { lane, expected: 1, found: 2, .. } if lane == "main"
            ),
            "{err}"
        );
    }

    #[test]
    fn a_duplicate_sequence_is_refused_like_a_gap() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(
            &dir,
            &[
                (RequestLane::MAIN, 0),
                (RequestLane::MAIN, 0),
                (RequestLane::MAIN, 1),
            ],
        );
        let err = LaneTable::build(&cassette).unwrap_err();
        assert!(
            matches!(
                &err,
                ReplayError::SequenceGap {
                    expected: 1,
                    found: 0,
                    ..
                }
            ),
            "{err}"
        );
    }

    /// A hole on a side lane condemns the tape just as loudly as one on
    /// the main lane; the run read that answer too.
    #[test]
    fn a_gap_on_a_side_lane_is_refused_too() {
        let dir = tempfile::tempdir().unwrap();
        let cassette = taped(
            &dir,
            &[(RequestLane::MAIN, 0), (RequestLane::named("title"), 1)],
        );
        let err = LaneTable::build(&cassette).unwrap_err();
        assert!(
            matches!(
                &err,
                ReplayError::SequenceGap { lane, expected: 0, found: 1, .. } if lane == "title"
            ),
            "{err}"
        );
    }
}
