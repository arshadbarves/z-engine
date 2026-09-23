//! The verification seam at the main agent's stop boundary. Phase 4 only
//! computes the badge; the checks-running verifier (auto/strict modes)
//! replaces [`BadgeOnly`] behind the same trait.

use std::fmt;

use async_trait::async_trait;
use z_engine_protocol::VerificationOutcome;

/// What the stop boundary does after verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StopVerdict {
    /// End the turn with this badge.
    Done(VerificationOutcome),
    /// Continue the run with this reminder (failed checks to fix).
    #[expect(
        dead_code,
        reason = "auto/strict verification continues turns (phase 8)"
    )]
    Continue { reminder: String },
}

#[async_trait]
pub(crate) trait Verifier: Send + Sync + fmt::Debug {
    /// `mutated`: files changed during this turn. `continuations`: how
    /// often this verifier already continued the turn.
    async fn at_stop(&self, mutated: bool, continuations: u32) -> StopVerdict;
}

/// Report-only verification: no checks run, so changes stay unverified.
#[derive(Debug, Default)]
pub(crate) struct BadgeOnly;

#[async_trait]
impl Verifier for BadgeOnly {
    async fn at_stop(&self, mutated: bool, _continuations: u32) -> StopVerdict {
        StopVerdict::Done(badge(mutated))
    }
}

/// The badge of a turn that ends without running checks.
pub(crate) fn badge(mutated: bool) -> VerificationOutcome {
    if mutated {
        VerificationOutcome::Unverified {
            reason: "no checks recorded".to_string(),
        }
    } else {
        VerificationOutcome::NotApplicable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn badge_depends_only_on_mutation() {
        assert_eq!(
            BadgeOnly.at_stop(false, 0).await,
            StopVerdict::Done(VerificationOutcome::NotApplicable)
        );
        assert!(matches!(
            BadgeOnly.at_stop(true, 0).await,
            StopVerdict::Done(VerificationOutcome::Unverified { .. })
        ));
    }
}
