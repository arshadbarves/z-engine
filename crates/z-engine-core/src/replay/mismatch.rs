//! Where and how a replay stopped being the run it was replaying.
//!
//! A divergence is reported the same way whatever caused it: the lane,
//! the position *in that lane's* recorded sequence, and both request
//! bodies so the difference can be read rather than guessed at. The
//! position is always the recorded sequence number, never a line
//! number or a whole-tape index, so "diverged at request 3" means the
//! same thing in the error, in the event, and on the tape.

use z_engine_provider::RequestLane;

use super::entry::{ProviderExchange, canonical_request};

/// Where and how a replay stopped being the run it was replaying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayMismatch {
    /// Which caller diverged.
    pub lane: String,
    /// Position in that lane's recorded request sequence (0-based).
    pub sequence: u64,
    /// Hash of the request recorded at that position; empty when the
    /// cassette holds no request there at all.
    pub expected_hash: String,
    /// Hash of the request this run actually made.
    pub actual_hash: String,
    /// What diverged, in words.
    pub detail: String,
    /// The recorded request, serialized, for diffing against `actual`.
    pub expected: Option<String>,
    /// The request this run actually made, serialized.
    pub actual: String,
}

impl ReplayMismatch {
    /// A request that could not be serialized cannot be matched, and an
    /// unmatchable request is a divergence like any other.
    pub(super) fn unserializable(lane: &RequestLane, sequence: u64, err: impl ToString) -> Self {
        Self {
            lane: lane.label().to_string(),
            sequence,
            expected_hash: String::new(),
            actual_hash: String::new(),
            detail: format!(
                "request could not be serialized for matching: {}",
                err.to_string()
            ),
            expected: None,
            actual: String::new(),
        }
    }

    /// This lane made more requests this time than it recorded. Reported
    /// per lane: another lane's length has nothing to do with it.
    pub(super) fn past_the_end(
        lane: &RequestLane,
        sequence: u64,
        recorded: usize,
        actual_hash: String,
        actual: String,
    ) -> Self {
        Self {
            lane: lane.label().to_string(),
            sequence,
            expected_hash: String::new(),
            actual_hash,
            detail: format!(
                "the cassette holds {recorded} request(s) on lane {}; this run made at least {}",
                lane.label(),
                sequence + 1
            ),
            expected: None,
            actual,
        }
    }

    /// The recorded request and this one are not the same bytes.
    pub(super) fn different(
        lane: &RequestLane,
        sequence: u64,
        exchange: &ProviderExchange,
        actual_hash: String,
        actual: String,
    ) -> Self {
        Self {
            lane: lane.label().to_string(),
            sequence,
            detail: format!(
                "request does not match the recording (expected {}, got {actual_hash})",
                exchange.request_hash
            ),
            expected_hash: exchange.request_hash.clone(),
            actual_hash,
            expected: canonical_request(&exchange.request)
                .map(|(text, _)| text)
                .ok(),
            actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z_engine_provider::{ChatMessage, ChatRequest};

    fn exchange() -> ProviderExchange {
        let request = ChatRequest::new("m", vec![ChatMessage::user("hi")]);
        ProviderExchange {
            lane: RequestLane::MAIN,
            sequence: 0,
            request_hash: "recorded-hash".into(),
            request,
            events: Vec::new(),
            error: None,
        }
    }

    /// The number in the message is the one the caller can look up on
    /// the tape, so an off-by-one here is a wrong report.
    #[test]
    fn running_past_the_end_counts_requests_not_positions() {
        let lane = RequestLane::named("title");
        let m = ReplayMismatch::past_the_end(&lane, 2, 2, "h".into(), "{}".into());
        assert_eq!(m.lane, "title");
        assert_eq!(m.sequence, 2);
        assert!(
            m.detail.contains("holds 2 request(s) on lane title"),
            "{m:?}"
        );
        assert!(m.detail.contains("at least 3"), "{m:?}");
        assert!(m.expected.is_none(), "there is nothing recorded to show");
    }

    #[test]
    fn a_difference_carries_both_sides_for_reading() {
        let m = ReplayMismatch::different(
            &RequestLane::MAIN,
            1,
            &exchange(),
            "actual-hash".into(),
            "{}".into(),
        );
        assert_eq!(m.expected_hash, "recorded-hash");
        assert_eq!(m.actual_hash, "actual-hash");
        assert!(m.expected.is_some(), "the recorded request is shown too");
        assert!(m.detail.contains("recorded-hash") && m.detail.contains("actual-hash"));
    }

    #[test]
    fn an_unserializable_request_is_a_divergence_at_its_own_position() {
        let m = ReplayMismatch::unserializable(&RequestLane::MAIN, 4, "not json");
        assert_eq!(m.sequence, 4);
        assert!(m.detail.contains("not json"));
    }
}
