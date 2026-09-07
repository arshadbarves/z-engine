//! Which logical stream a request belongs to.
//!
//! A provider handle is shared: the turn loop, a session-title request, a
//! reviewer pass and any number of sub-agents all call the same
//! [`ChatProvider`](crate::ChatProvider) concurrently. Nothing about the
//! wire distinguishes those calls, so anything that records or reproduces
//! traffic has only *arrival order* to go on — and arrival order between
//! independent callers is the scheduler's, not the run's.
//!
//! A lane is the caller's own answer to "which conversation is this?".
//! It is carried beside the request, never inside it: the bytes on the
//! wire are unchanged, and this crate never interprets the label. What
//! the lanes are called, and which caller uses which, is the caller's
//! policy.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// An opaque label identifying one logical stream of requests.
///
/// Requests within a lane are ordered by the caller; requests in
/// different lanes have no order relative to each other.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestLane(Cow<'static, str>);

impl RequestLane {
    /// The lane a caller that never says otherwise is on.
    pub const MAIN: RequestLane = RequestLane(Cow::Borrowed("main"));

    /// Name a lane. An empty label is [`RequestLane::MAIN`]: a nameless
    /// lane would silently merge unrelated callers, which is the exact
    /// confusion lanes exist to prevent.
    pub fn named(label: impl Into<String>) -> Self {
        let label = label.into();
        if label.trim().is_empty() {
            return Self::MAIN;
        }
        Self(Cow::Owned(label))
    }

    /// Name the `ordinal`-th lane of a kind (`"sub"`, 2 → `"sub#2"`).
    pub fn indexed(kind: &str, ordinal: u64) -> Self {
        Self::named(format!("{kind}#{ordinal}"))
    }

    pub fn label(&self) -> &str {
        &self.0
    }

    pub fn is_main(&self) -> bool {
        self.0 == RequestLane::MAIN.0
    }
}

impl Default for RequestLane {
    fn default() -> Self {
        Self::MAIN
    }
}

impl std::fmt::Display for RequestLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for RequestLane {
    fn from(label: &str) -> Self {
        Self::named(label.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_lane_is_the_main_one() {
        assert_eq!(RequestLane::default(), RequestLane::MAIN);
        assert!(RequestLane::MAIN.is_main());
        assert_eq!(RequestLane::MAIN.label(), "main");
    }

    /// A blank label would put two unrelated callers on one lane, which
    /// is the interleaving lanes exist to stop.
    #[test]
    fn a_nameless_lane_is_the_main_lane_rather_than_a_new_empty_one() {
        assert_eq!(RequestLane::named("  "), RequestLane::MAIN);
        assert_eq!(RequestLane::named(""), RequestLane::MAIN);
    }

    #[test]
    fn lanes_are_distinct_by_label_and_ordinal() {
        assert_ne!(RequestLane::named("title"), RequestLane::MAIN);
        assert_ne!(
            RequestLane::indexed("sub", 0),
            RequestLane::indexed("sub", 1)
        );
        assert_eq!(RequestLane::indexed("sub", 2).label(), "sub#2");
    }

    /// The lane rides beside the request on the tape, so it has to
    /// survive JSON as the plain label it is.
    #[test]
    fn a_lane_round_trips_as_its_label() {
        let lane = RequestLane::named("review");
        let json = serde_json::to_string(&lane).unwrap();
        assert_eq!(json, "\"review\"");
        assert_eq!(serde_json::from_str::<RequestLane>(&json).unwrap(), lane);
    }
}
