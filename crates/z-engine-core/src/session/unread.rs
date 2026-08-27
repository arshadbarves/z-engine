//! Last turn-end that the user has not opened yet. Survives restart
//! because it is stored as JSONL alongside the transcript.

use super::SessionEvent;

/// `completed` / `aborted` after the latest turn-end, or `None` once an
/// `Ack` (session opened) has been recorded.
pub fn unread_outcome(events: &[SessionEvent]) -> Option<String> {
    let mut last: Option<String> = None;
    let mut acked = true;
    for ev in events {
        match ev {
            SessionEvent::Ack => acked = true,
            SessionEvent::TurnEnd { outcome } => {
                last = Some(outcome.clone());
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
}
