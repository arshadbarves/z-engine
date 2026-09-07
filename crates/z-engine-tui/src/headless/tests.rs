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

/// Long enough that no scripted feed reaches it, so a test that exercises
/// the deadline has to ask for it explicitly.
fn generous() -> Limits {
    Limits {
        deadline: Duration::from_secs(30),
    }
}

async fn run(events: Vec<Event>) -> anyhow::Result<()> {
    drive(
        Scripted::new(events),
        &Recorded::default(),
        false,
        generous(),
    )
    .await
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
            generous(),
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
        generous(),
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
        generous(),
    )
    .await
    .expect("denied approvals do not fail the run");
    assert_eq!(*approvals.denied.borrow(), vec![7]);
    assert!(approvals.approved.borrow().is_empty());
}

/// A turn a gate refused is not a finished turn: a one-shot run must
/// exit non-zero even though the model produced a confident final
/// answer, and the message must name the gate that refused.
#[tokio::test]
async fn a_blocked_turn_exits_non_zero_and_names_the_gate() {
    let err = run(vec![
        Event::TokenDelta("All done — everything passes.".into()),
        Event::TurnBlocked {
            gate: "completion".into(),
            reason: "verification did not pass: `cargo check` failed (exit 101)".into(),
            manifest_path: Some("/repo/.z-engine/runs/01ABC/verification.json".into()),
        },
    ])
    .await
    .expect_err("a blocked turn must not exit zero");

    let msg = err.to_string();
    assert!(msg.contains("completion gate blocked the turn"), "{msg}");
    assert!(msg.contains("cargo check"), "{msg}");
}

/// The refusal is terminal for the turn: nothing after it — including a
/// stray `TurnCompleted` — can turn a blocked run into a clean exit.
#[tokio::test]
async fn nothing_after_a_blocked_turn_can_make_it_succeed() {
    let err = run(vec![
        Event::TurnBlocked {
            gate: "completion".into(),
            reason: "nothing was verified".into(),
            manifest_path: None,
        },
        Event::TurnCompleted {
            prompt_tokens: 1,
            completion_tokens: 1,
        },
    ])
    .await
    .expect_err("a blocked turn must not exit zero");
    assert!(err.to_string().contains("nothing was verified"), "{err}");
}

/// The silent drop. A panicked or early-returning agent task drops the
/// event sender, and the runner used to read that closed channel as a
/// finished run — exiting zero with nothing verified, in the mode whose
/// whole promise is that it fails closed.
#[tokio::test]
async fn a_channel_that_closes_without_a_terminal_event_is_a_failure() {
    for prelude in [
        vec![],
        vec![Event::TurnStarted],
        vec![
            Event::TurnStarted,
            Event::TokenDelta("working on it".into()),
            Event::ToolCallFinished {
                name: "read_file".into(),
                ok: true,
                duration_ms: 4,
                summary: "read 40 lines".into(),
            },
        ],
    ] {
        let err = run(prelude)
            .await
            .expect_err("a dropped agent must not exit zero");
        let msg = err.to_string();
        assert!(msg.contains("without a verdict"), "{msg}");
        assert!(msg.contains("nothing was verified"), "{msg}");
    }
}

/// …and the clean exit still exists: only `TurnCompleted` grants it, and
/// a channel closing afterwards changes nothing.
#[tokio::test]
async fn an_explicit_completion_is_still_a_clean_exit() {
    run(vec![
        Event::TurnStarted,
        Event::TokenDelta("done".into()),
        Event::TurnCompleted {
            prompt_tokens: 10,
            completion_tokens: 20,
        },
    ])
    .await
    .expect("an explicitly completed turn exits zero");
}

/// A provider that accepted the request and then stopped answering keeps
/// the channel open forever. Without a global bound the run outlives the
/// CI job; with one it exits non-zero and says which knob raises it.
#[tokio::test]
async fn a_stalled_run_exits_on_its_deadline_instead_of_hanging() {
    let limits = Limits {
        deadline: Duration::from_millis(120),
    };
    let err = timeout(
        Duration::from_secs(5),
        drive(Live::new(vec![]), &Recorded::default(), false, limits),
    )
    .await
    .expect("the run deadline must be enforced, not merely declared")
    .expect_err("a run with no verdict must not exit zero");

    let msg = err.to_string();
    assert!(msg.contains("run deadline"), "{msg}");
    assert!(msg.contains("--timeout"), "{msg}");
}

/// The bound covers the whole run, not one quiet gap: a feed that keeps
/// talking without ever finishing still ends at the deadline.
#[tokio::test]
async fn a_chatty_run_that_never_finishes_still_ends_at_the_deadline() {
    let limits = Limits {
        deadline: Duration::from_millis(150),
    };
    let chatter: Vec<(Duration, Event)> = (0..50)
        .map(|i| {
            (
                Duration::from_millis(10),
                Event::StatusNote(format!("still working ({i})")),
            )
        })
        .collect();
    let err = timeout(
        Duration::from_secs(5),
        drive(Live::new(chatter), &Recorded::default(), false, limits),
    )
    .await
    .expect("the deadline bounds the run, not the gap between events")
    .expect_err("no verdict means no clean exit");
    assert!(err.to_string().contains("run deadline"), "{err}");
}

/// The default is finite: a runner whose ceiling could be absent would
/// only be bounded when someone remembered to ask.
#[test]
fn the_run_deadline_defaults_to_a_finite_bound() {
    assert_eq!(Limits::default().deadline, DEFAULT_RUN_DEADLINE);
    assert_eq!(Limits::from_secs(None).deadline, DEFAULT_RUN_DEADLINE);
    assert_eq!(
        Limits::from_secs(Some(90)).deadline,
        Duration::from_secs(90)
    );
}
