//! The stop boundary: the model ended a response without tool calls.
//! `Stop`/`SubagentStop` hooks may continue the run (at most five times
//! per turn), queued steering continues it, and for the main agent the
//! verifier computes the badge or asks for another round.

use serde_json::json;
use z_engine_context::wrap_reminder;
use z_engine_protocol::{ContentBlock, NoticeLevel, VerificationOutcome};

use super::reminders::take_steering;
use super::spec::RunContext;
use crate::hooks::{HookEvent, HookInput, run_hooks};
use crate::verify::StopVerdict;

const MAX_HOOK_CONTINUATIONS: u32 = 5;

#[derive(Debug, Default)]
pub(crate) struct StopCounters {
    pub hook_continuations: u32,
    pub verify_continuations: u32,
}

#[derive(Debug)]
pub(crate) enum StopAction {
    /// Send these blocks as a new user message and keep going.
    Continue {
        content: Vec<ContentBlock>,
        steering: bool,
    },
    End {
        verification: Option<VerificationOutcome>,
    },
}

pub(crate) async fn stop_boundary(
    ctx: &RunContext,
    counters: &mut StopCounters,
    mutated: bool,
) -> StopAction {
    let main = ctx.spec.is_main();
    let (event, mut input) = if main {
        (HookEvent::Stop, HookInput::new())
    } else {
        let agent = HookInput::new().with("agent_id", json!(ctx.spec.agent_id));
        (HookEvent::SubagentStop, agent)
    };
    input = input.with("stop_hook_active", json!(counters.hook_continuations > 0));
    let outcome = run_hooks(
        &ctx.core.hook_env(),
        &ctx.core.events,
        event,
        input,
        &ctx.cancel,
    )
    .await;
    if let Some(reason) = outcome.stop {
        ctx.core.events.notice(
            NoticeLevel::Info,
            format!("A stop hook ended the turn: {reason}"),
        );
        return StopAction::End { verification: None };
    }
    if let Some(reason) = outcome.blocked {
        if counters.hook_continuations < MAX_HOOK_CONTINUATIONS {
            counters.hook_continuations += 1;
            let feedback = format!("{} hook feedback:\n{reason}", event.name());
            return StopAction::Continue {
                content: vec![ContentBlock::text(wrap_reminder(&feedback))],
                steering: false,
            };
        }
        ctx.core.events.notice(
            NoticeLevel::Warn,
            format!("Stop hooks continued this turn {MAX_HOOK_CONTINUATIONS} times; ending it."),
        );
    }
    let steering = take_steering(ctx);
    if !steering.is_empty() {
        return StopAction::Continue {
            content: steering,
            steering: true,
        };
    }
    if !main {
        return StopAction::End { verification: None };
    }
    match ctx
        .core
        .verifier
        .at_stop(mutated, counters.verify_continuations)
        .await
    {
        StopVerdict::Done(outcome) => StopAction::End {
            verification: Some(outcome),
        },
        StopVerdict::Continue { reminder } => {
            counters.verify_continuations += 1;
            StopAction::Continue {
                content: vec![ContentBlock::text(reminder)],
                steering: false,
            }
        }
    }
}
