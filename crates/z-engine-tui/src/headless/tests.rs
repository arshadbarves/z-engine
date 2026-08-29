use std::cell::RefCell;

use super::*;

/// A scripted feed: the events a run produced, in order, then the
/// closed channel.
struct Scripted(std::collections::VecDeque<Event>);

impl Scripted {
    fn new(events: Vec<Event>) -> Self {
        Self(events.into())
    }
}

impl Events for Scripted {
    async fn next(&mut self) -> Option<Event> {
        self.0.pop_front()
    }
}

/// A feed that behaves like a live channel: it takes time to deliver
/// an event and it never closes. `Scripted` closes the moment it runs
/// out, which would end a wait for a verdict for the wrong reason and
/// leave a hang undetectable.
struct Live(std::collections::VecDeque<(Duration, Event)>);

impl Live {
    fn new(events: Vec<(Duration, Event)>) -> Self {
        Self(events.into())
    }
}

impl Events for Live {
    async fn next(&mut self) -> Option<Event> {
        match self.0.pop_front() {
            Some((after, event)) => {
                tokio::time::sleep(after).await;
                Some(event)
            }
            // Still connected, still quiet — the caller must not wait
            // on this forever.
            None => std::future::pending().await,
        }
    }
}

#[derive(Default)]
struct Recorded {
    approved: RefCell<Vec<u64>>,
    denied: RefCell<Vec<u64>>,
}

impl Approvals for Recorded {
    fn approve_once(&self, id: u64) {
        self.approved.borrow_mut().push(id);
    }

    fn deny(&self, id: u64) {
        self.denied.borrow_mut().push(id);
    }
}

async fn run(events: Vec<Event>) -> anyhow::Result<()> {
    drive(Scripted::new(events), &Recorded::default(), false).await
}

/// The refusal a guarded run emits: detail first, verdict second.
/// Exiting on the detail would make the verdict unreachable, so the
/// message and the exit must come from `RunBlocked`.
#[tokio::test]
async fn a_blocked_run_exits_on_the_verdict_not_the_error_before_it() {
    let err = run(vec![
        Event::Error("guarded mode unavailable: no storage; refusing to run ungoverned".into()),
        Event::RunBlocked {
            reason: "guarded mode unavailable: no storage; refusing to run ungoverned".into(),
        },
    ])
    .await
    .expect_err("a refused run must not exit zero");

    let msg = err.to_string();
    assert!(msg.starts_with("run blocked: "), "{msg}");
    assert!(msg.contains("refusing to run ungoverned"), "{msg}");
}

/// …and the same when the two are not adjacent: the verdict decides.
#[tokio::test]
async fn events_between_the_error_and_the_verdict_do_not_hide_it() {
    let err = run(vec![
        Event::Error("boom".into()),
        Event::UsageUpdated {
            prompt_tokens: 1,
            completion_tokens: 2,
        },
        Event::RunBlocked {
            reason: "guarded mode unavailable".into(),
        },
    ])
    .await
    .expect_err("blocked");
    assert_eq!(err.to_string(), "run blocked: guarded mode unavailable");
}

/// An ordinary error is still an ordinary error: it keeps its own
/// message, and waiting for a verdict that never comes must end.
#[tokio::test]
async fn an_unrelated_error_still_fails_with_its_own_message() {
    let err = run(vec![Event::Error("provider init failed: bad url".into())])
        .await
        .expect_err("errors fail the run");
    assert_eq!(err.to_string(), "provider init failed: bad url");
}

/// …and that holds on a channel that stays *open*. Closing is what
/// ends the wait above; here nothing closes and nothing follows, so
/// only the bound on the wait can end the run. Failing this test
/// means the runner hangs a CI job instead of exiting non-zero.
#[tokio::test]
async fn an_error_no_verdict_ever_explains_does_not_hang_the_run() {
    let err = timeout(
        Duration::from_secs(5),
        drive(
            Live::new(vec![(
                Duration::ZERO,
                Event::Error("provider init failed: bad url".into()),
            )]),
            &Recorded::default(),
            false,
        ),
    )
    .await
    .expect("the wait for a verdict is bounded")
    .expect_err("errors fail the run");
    assert_eq!(err.to_string(), "provider init failed: bad url");
}

/// The other half of that bound: a verdict that needs a scheduling
/// moment to arrive still decides the exit, so the grace window is
/// doing real work rather than only reading events already queued.
#[tokio::test]
async fn a_verdict_that_arrives_a_moment_later_still_decides_the_exit() {
    let err = drive(
        Live::new(vec![
            (Duration::ZERO, Event::Error("boom".into())),
            (
                BLOCKED_VERDICT_GRACE / 3,
                Event::RunBlocked {
                    reason: "guarded mode unavailable".into(),
                },
            ),
        ]),
        &Recorded::default(),
        false,
    )
    .await
    .expect_err("blocked");
    assert_eq!(err.to_string(), "run blocked: guarded mode unavailable");
}

/// A blocked verdict with no error before it is terminal on its own.
#[tokio::test]
async fn a_bare_verdict_is_terminal() {
    let err = run(vec![
        Event::RunBlocked {
            reason: "refused".into(),
        },
        Event::TurnCompleted {
            prompt_tokens: 0,
            completion_tokens: 0,
        },
    ])
    .await
    .expect_err("blocked");
    assert_eq!(err.to_string(), "run blocked: refused");
}

#[tokio::test]
async fn a_finished_turn_still_exits_zero() {
    run(vec![
        Event::TurnStarted,
        Event::TurnCompleted {
            prompt_tokens: 3,
            completion_tokens: 4,
        },
    ])
    .await
    .expect("a completed turn is a clean exit");
}

/// Non-interactive runs deny what they cannot ask about, and that
/// path must survive the restructuring.
#[tokio::test]
async fn approvals_are_denied_without_a_terminal_and_the_run_continues() {
    let approvals = Recorded::default();
    drive(
        Scripted::new(vec![
            Event::ApprovalRequired {
                id: 7,
                tool: "bash".into(),
                input_preview: "rm -rf build".into(),
                suggested_rule: None,
                detail_preview: None,
                can_persist: false,
                bash_command: Some("rm -rf build".into()),
            },
            Event::TurnCompleted {
                prompt_tokens: 0,
                completion_tokens: 0,
            },
        ]),
        &approvals,
        false,
    )
    .await
    .expect("denied approvals do not fail the run");
    assert_eq!(*approvals.denied.borrow(), vec![7]);
    assert!(approvals.approved.borrow().is_empty());
}
