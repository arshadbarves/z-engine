//! Last turn-end that the user has not opened yet. Survives restart
//! because it is stored as JSONL alongside the transcript.

use super::SessionEvent;
use crate::verification::TaskStatus;

/// Legacy turn outcome or the latest task status, cleared by `Ack`.
/// A completed provider turn never overrides a verification report.
pub fn unread_outcome(events: &[SessionEvent]) -> Option<String> {
    let mut last: Option<String> = None;
    let mut acked = true;
    let mut task_outcome = None;
    for ev in events {
        match ev {
            SessionEvent::Ack => acked = true,
            SessionEvent::UserMsg { .. } => {
                task_outcome = None;
                last = None;
                acked = true;
            }
            SessionEvent::TaskUpdated { report } => {
                let outcome = match report.status {
                    TaskStatus::Complete => "complete",
                    TaskStatus::Running
                    | TaskStatus::NeedsVerification
                    | TaskStatus::Interrupted => "interrupted",
                    TaskStatus::Blocked => "blocked",
                    TaskStatus::Stopped => "stopped",
                    TaskStatus::Unassessed => "unassessed",
                    TaskStatus::Stale => "stale",
                };
                task_outcome = Some(outcome.to_string());
                last = task_outcome.clone();
                acked = false;
            }
            SessionEvent::TurnEnd { outcome } => {
                last = task_outcome.clone().or_else(|| Some(outcome.clone()));
                acked = false;
            }
            _ => {}
        }
    }
    if acked { None } else { last }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionEvent;

    fn end(outcome: &str) -> SessionEvent {
        SessionEvent::TurnEnd {
            outcome: outcome.into(),
        }
    }

    #[test]
    fn stays_until_ack_then_clears() {
        assert_eq!(
            unread_outcome(&[end("completed")]).as_deref(),
            Some("completed")
        );
        assert_eq!(unread_outcome(&[end("completed"), SessionEvent::Ack]), None);
        assert_eq!(
            unread_outcome(&[end("completed"), SessionEvent::Ack, end("aborted")]).as_deref(),
            Some("aborted")
        );
    }

    #[test]
    fn completed_turn_does_not_override_failed_or_interrupted_verification() {
        for (status, expected) in [
            (TaskStatus::Complete, "complete"),
            (TaskStatus::Blocked, "blocked"),
            (TaskStatus::Stale, "stale"),
            (TaskStatus::Running, "interrupted"),
        ] {
            let mut event = crate::session::tests::report_event();
            let SessionEvent::TaskUpdated { report } = &mut event else {
                unreachable!()
            };
            report.status = status;
            assert_eq!(
                unread_outcome(&[event.clone(), end("completed")]).as_deref(),
                Some(expected)
            );
            assert_eq!(
                unread_outcome(&[event, end("completed"), SessionEvent::Ack]),
                None
            );
        }
    }
}
