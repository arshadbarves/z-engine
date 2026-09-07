//! The barrier between "the turn moved on" and "the tape is whole".
//!
//! An exchange is appended when its stream *ends*, which can be after
//! the turn that issued it has moved on — the consumer stops reading at
//! the finish event, not at channel close. Side requests go further
//! still: a session title runs beside the turn and only registers its
//! forwarding task once it starts, so draining the list once can miss
//! work that appeared while it drained.
//!
//! Without this barrier a run's own last request can be missing from
//! its record, and the tally can be snapshotted before its tokens are
//! counted: a tape that is quietly short is worse than no tape.

use std::sync::Mutex;
use std::time::Duration;

use tokio::task::JoinHandle;

use super::fault::RecordingFault;

/// How long [`InFlight::settle`] waits for one stream to finish
/// reporting before recording that it did not. Bounded so a wedged
/// provider costs the run a known fault rather than the turn.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);

/// How many times [`InFlight::settle`] re-drains the list. A task can
/// register another while it finishes (a side request tapes through the
/// same recorder), so one pass is not enough; an unbounded number of
/// passes would be a way to never return.
const SETTLE_PASSES: usize = 8;

/// The forwarding tasks that still owe the tape an exchange.
#[derive(Default)]
pub(super) struct InFlight {
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl InFlight {
    pub(super) fn track(&self, handle: JoinHandle<()>) {
        self.handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(handle);
    }

    /// Wait until every exchange in flight has reached the tape,
    /// reporting the first one that did not arrive.
    pub(super) async fn settle(&self) -> Option<RecordingFault> {
        let mut lost = None;
        for _ in 0..SETTLE_PASSES {
            let handles: Vec<JoinHandle<()>> =
                std::mem::take(&mut *self.handles.lock().unwrap_or_else(|e| e.into_inner()));
            if handles.is_empty() {
                return lost;
            }
            for handle in handles {
                let detail = match tokio::time::timeout(SETTLE_TIMEOUT, handle).await {
                    Ok(Ok(())) => continue,
                    Ok(Err(err)) => format!("a recording task failed: {err}"),
                    Err(_) => format!("a recording task did not finish within {SETTLE_TIMEOUT:?}"),
                };
                lost.get_or_insert(RecordingFault {
                    kind: "exchange".to_string(),
                    detail,
                });
            }
        }
        Some(lost.unwrap_or(RecordingFault {
            kind: "exchange".to_string(),
            detail: format!("recording work was still arriving after {SETTLE_PASSES} passes"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nothing_in_flight_settles_clean() {
        assert!(InFlight::default().settle().await.is_none());
    }

    #[tokio::test]
    async fn a_tracked_task_is_waited_for_and_not_faulted() {
        let flight = InFlight::default();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        flight.track(tokio::spawn(async move {
            let _ = rx.await;
        }));
        tokio::spawn(async move {
            let _ = tx.send(());
        });
        assert!(flight.settle().await.is_none());
    }

    #[tokio::test]
    async fn a_task_that_panicked_is_a_lost_exchange() {
        let flight = InFlight::default();
        flight.track(tokio::spawn(async { panic!("no tape for you") }));
        let fault = flight.settle().await.expect("a panicked task is a fault");
        assert_eq!(fault.kind, "exchange");
    }
}
