use std::collections::BTreeSet;

use crate::{
    Boundary, MAX_CONTINUATIONS, SupervisionAction, SupervisionError, SupervisionReport, WorkAction,
};

#[derive(Debug)]
pub struct Supervisor {
    limit: u32,
    continuations: u32,
    seen: BTreeSet<String>,
}

impl Supervisor {
    pub fn new(limit: u32) -> Result<Self, SupervisionError> {
        if limit > MAX_CONTINUATIONS {
            return Err(SupervisionError::InvalidLimit(limit));
        }
        Ok(Self {
            limit,
            continuations: 0,
            seen: BTreeSet::new(),
        })
    }

    pub fn decide(&mut self, boundary: Boundary) -> SupervisionReport {
        let (action, reason) = match boundary {
            Boundary::Cancelled => (SupervisionAction::Stopped, "Cancellation requested.".into()),
            Boundary::Blocked(reason) => (SupervisionAction::Blocked, reason),
            Boundary::Complete => (
                SupervisionAction::Complete,
                "Current evidence satisfies the completion gate; durable finalization is next."
                    .into(),
            ),
            Boundary::Ineligible => (
                SupervisionAction::Idle,
                "No supported code-changing or verification activity requires continuation.".into(),
            ),
            Boundary::Incomplete { progress_key, next } => {
                if self.limit == 0 {
                    (
                        SupervisionAction::Idle,
                        "Automatic task continuation is disabled.".into(),
                    )
                } else if self.continuations >= self.limit {
                    (
                        SupervisionAction::Blocked,
                        "Task continuation budget exhausted; work remains incomplete.".into(),
                    )
                } else if !self.seen.insert(progress_key) {
                    (
                        SupervisionAction::Blocked,
                        "A previously observed workspace/check state recurred without verified completion. Human input or a different approach is needed.".into(),
                    )
                } else {
                    self.continuations += 1;
                    match next {
                        WorkAction::Continue => (
                            SupervisionAction::Continue,
                            "Requirement coverage is still missing for current evidence.".into(),
                        ),
                        WorkAction::Verify => (
                            SupervisionAction::Verify,
                            "Changes require current, applicable verification evidence.".into(),
                        ),
                        WorkAction::Repair => (
                            SupervisionAction::Repair,
                            "Recorded checks failed; investigate before attempting a repair."
                                .into(),
                        ),
                    }
                }
            }
        };
        SupervisionReport {
            continuations: self.continuations,
            max_continuations: self.limit,
            last_action: action,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incomplete(key: &str) -> Boundary {
        Boundary::Incomplete {
            progress_key: key.into(),
            next: WorkAction::Verify,
        }
    }

    #[test]
    fn useful_progress_continues_under_one_budget() {
        let mut supervisor = Supervisor::new(2).unwrap();
        assert_eq!(supervisor.decide(incomplete("v1")).continuations, 1);
        assert_eq!(supervisor.decide(incomplete("v2")).continuations, 2);
        let exhausted = supervisor.decide(incomplete("v3"));
        assert_eq!(exhausted.last_action, SupervisionAction::Blocked);
        assert_eq!(exhausted.continuations, 2);
        assert_eq!(
            supervisor.decide(Boundary::Complete).last_action,
            SupervisionAction::Complete
        );
    }

    #[test]
    fn repeated_and_cyclic_states_are_not_progress() {
        let mut supervisor = Supervisor::new(10).unwrap();
        supervisor.decide(incomplete("v1"));
        supervisor.decide(incomplete("v2"));
        assert_eq!(
            supervisor.decide(incomplete("v1")).last_action,
            SupervisionAction::Blocked
        );
    }

    #[test]
    fn denial_cancellation_and_ineligible_tasks_never_continue() {
        let mut supervisor = Supervisor::new(3).unwrap();
        for boundary in [
            Boundary::Cancelled,
            Boundary::Blocked("Permission denied".into()),
            Boundary::Ineligible,
        ] {
            let result = supervisor.decide(boundary);
            assert!(!result.last_action.continues());
            assert_eq!(result.continuations, 0);
        }
    }

    #[test]
    fn disabled_and_invalid_limits_are_explicit() {
        assert!(Supervisor::new(11).is_err());
        let mut supervisor = Supervisor::new(0).unwrap();
        assert_eq!(
            supervisor.decide(incomplete("v1")).last_action,
            SupervisionAction::Idle
        );
    }

    #[test]
    fn decisions_have_stable_wire_shape() {
        let mut supervisor = Supervisor::new(3).unwrap();
        let value = serde_json::to_value(supervisor.decide(incomplete("v1"))).unwrap();
        assert_eq!(value["lastAction"], "verify");
        assert_eq!(value["maxContinuations"], 3);
        assert_eq!(value["continuations"], 1);
    }
}
