//! Which logical request lanes this agent uses, and what each is for.
//!
//! One provider handle serves several independent callers: the turn
//! loop, the post-edit reviewer, the compaction summarizer, the session
//! titler, and any number of sub-agents. Nothing orders those callers
//! against each other — the titler runs *beside* the turn it names — so
//! a recorder that saw only arrival order would tape a different
//! interleaving every run, and a replay matching in that order would
//! diverge on a run that did nothing wrong.
//!
//! Naming the lane makes each caller's own sequence the thing that is
//! matched. What the labels are is policy, and policy lives here rather
//! than in the transport.

use z_engine_provider::RequestLane;

use crate::replay::content_hash;

/// The post-edit reviewer's lane.
pub(super) fn review() -> RequestLane {
    RequestLane::named("review")
}

/// The compaction summarizer's lane.
pub(super) fn compact() -> RequestLane {
    RequestLane::named("compact")
}

/// The session titler's lane. It runs beside the first turn rather than
/// inside it, which is exactly why it needs one.
pub(super) fn title() -> RequestLane {
    RequestLane::named("title")
}

/// A sub-agent's lane.
///
/// Sub-agents are spawned concurrently, so arrival order is not theirs
/// to give. The lane is derived from what the parent *asked* — the
/// prompt — plus which delegation of that prompt this is, so the same
/// round of delegations names its lanes the same way every run.
///
/// Two sub-agents launched with byte-identical prompts in one round
/// still race for their ordinals; that ambiguity is the caller's, and a
/// replay of it diverges loudly rather than quietly.
pub(super) fn subagent(prompt: &str, ordinal: u64) -> RequestLane {
    let digest = content_hash(prompt.as_bytes());
    RequestLane::indexed(&format!("sub:{}", &digest[..8.min(digest.len())]), ordinal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn each_side_caller_gets_its_own_lane() {
        let lanes = [review(), compact(), title(), RequestLane::MAIN];
        for (i, a) in lanes.iter().enumerate() {
            for b in &lanes[i + 1..] {
                assert_ne!(a, b, "side callers must not share a lane");
            }
        }
    }

    /// The same delegation names the same lane whichever run it is, so a
    /// recording and its replay agree without either counting on when
    /// the sub-agent happened to start.
    #[test]
    fn a_subagent_lane_is_derived_from_its_prompt_not_its_arrival() {
        assert_eq!(subagent("find the bug", 0), subagent("find the bug", 0));
        assert_ne!(subagent("find the bug", 0), subagent("find the bug", 1));
        assert_ne!(subagent("find the bug", 0), subagent("write the doc", 0));
        assert!(subagent("find the bug", 0).label().starts_with("sub:"));
    }

    /// Ordinals come from a shared per-prompt counter, so delegations
    /// launched in any order still cover 0..n exactly once.
    #[test]
    fn ordinals_are_claimed_per_prompt() {
        let counters: Arc<Mutex<HashMap<String, u64>>> = Arc::default();
        let claim = |prompt: &str| -> u64 {
            let mut map = counters.lock().unwrap();
            let slot = map.entry(prompt.to_string()).or_insert(0);
            let n = *slot;
            *slot += 1;
            n
        };
        assert_eq!(claim("a"), 0);
        assert_eq!(claim("b"), 0);
        assert_eq!(claim("a"), 1);
    }
}
