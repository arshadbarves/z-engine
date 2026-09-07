//! One-shot headless runner: submit a task, stream plain-text events,
//! exit non-zero on failure. Acceptance/CI companion to the TUI.

use std::time::Duration;

use tokio::time::{Instant, timeout};
use z_engine_core::agent::{AgentHandle, ApprovalDecision, Event, EventRx};

pub async fn run_one_shot(
    handle: AgentHandle,
    ev: EventRx,
    task: &str,
    auto_approve: bool,
    limits: Limits,
) -> anyhow::Result<()> {
    eprintln!("zengine --headless · task: {task}");
    handle.submit(task.to_string());
    drive(ev, &handle, auto_approve, limits).await
}

/// Default ceiling on a whole headless run. Generous enough for a cold
/// workspace compile plus a multi-turn task, finite so an unattended CI
/// job cannot be held open by a provider that stopped answering.
pub const DEFAULT_RUN_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// What bounds one headless run. A separate value rather than a constant
/// so CI can shorten it and tests can prove the bound exists.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Wall-clock ceiling for the whole run, verdict included.
    pub deadline: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            deadline: DEFAULT_RUN_DEADLINE,
        }
    }
}

impl Limits {
    /// Bound the run at `secs`, or keep the default when unset.
    pub fn from_secs(secs: Option<u64>) -> Self {
        match secs {
            Some(secs) => Self {
                deadline: Duration::from_secs(secs),
            },
            None => Self::default(),
        }
    }
}

/// The event feed, as the runner needs it. `EventRx` is the real one; a
/// test scripts the sequence instead, so exit semantics can be proven
/// without an agent, a provider, or a network.
trait Events {
    fn next(&mut self) -> impl std::future::Future<Output = Option<Event>>;
}

impl Events for EventRx {
    async fn next(&mut self) -> Option<Event> {
        self.recv().await
    }
}

/// The two answers an approval prompt can get.
trait Approvals {
    fn approve_once(&self, id: u64);
    fn deny(&self, id: u64);
}

impl Approvals for AgentHandle {
    fn approve_once(&self, id: u64) {
        self.approve(id, ApprovalDecision::Once);
    }

    fn deny(&self, id: u64) {
        AgentHandle::deny(self, id);
    }
}

/// How long a fatal `Error` waits for a `RunBlocked` that would explain
/// it. The core sends the pair back to back, so this is a scheduling
/// margin rather than a poll interval — and it is bounded, so a run that
/// only errors still exits immediately afterwards.
const BLOCKED_VERDICT_GRACE: Duration = Duration::from_millis(150);

/// Drive one run to its verdict.
///
/// Two ways out that are *not* success, and both used to be: an agent
/// that drops the event channel without a terminal event has verified
/// nothing, and a run that never produces one at all must not outlive
/// its deadline. Exiting zero for either would make a fail-closed mode
/// report a clean run for work that never happened.
async fn drive<E: Events>(
    mut ev: E,
    approvals: &impl Approvals,
    auto_approve: bool,
    limits: Limits,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + limits.deadline;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok(next) = timeout(remaining, ev.next()).await else {
            anyhow::bail!(
                "no verdict within the {}s run deadline; nothing was verified \
                 (raise it with --timeout SECS)",
                limits.deadline.as_secs()
            );
        };
        match next {
            Some(Event::ReasoningDelta(_)) => {
                eprintln!("[thinking…]");
            }
            Some(Event::TokenDelta(t)) => {
                use std::io::Write;
                // `print!` panics on EPIPE (closed stdout, e.g. CI piping
                // into `head`); write + explicit error instead.
                if std::io::stdout().write_all(t.as_bytes()).is_err() {
                    anyhow::bail!("stdout closed");
                }
                std::io::stdout().flush().ok();
            }
            Some(Event::ToolOutputDelta { text, .. }) => {
                eprintln!("[out] {text}");
            }
            Some(Event::ToolCallStarted { name, preview }) => {
                eprintln!("\n[tool] {name} {preview}");
            }
            Some(Event::ToolCallFinished { ok, summary, .. }) => {
                eprintln!("[tool →] {}{summary}", if ok { "" } else { "ERR " });
            }
            Some(Event::ApprovalRequired {
                id,
                tool,
                input_preview,
                detail_preview,
                ..
            }) => {
                if let Some(diff) = &detail_preview {
                    for line in diff.lines().take(12) {
                        eprintln!("  {line}");
                    }
                }
                if auto_approve {
                    eprintln!("[auto-approved] {tool} {input_preview}");
                    approvals.approve_once(id);
                } else {
                    eprintln!(
                        "[denied: non-interactive] {tool} {input_preview}\n\
                         hint: rerun with --auto-approve to allow gated tools"
                    );
                    approvals.deny(id);
                }
            }
            Some(Event::UsageUpdated { .. }) => {}
            Some(Event::StatusNote(s)) => eprintln!("[status] {s}"),
            Some(Event::TurnStarted) => {}
            Some(Event::TurnCompleted {
                prompt_tokens,
                completion_tokens,
            }) => {
                eprintln!("\n[done] tokens: prompt={prompt_tokens} completion={completion_tokens}");
                return Ok(());
            }
            Some(Event::TurnAborted) => {
                anyhow::bail!("aborted");
            }
            // A gate refused to accept the turn as finished. The work was
            // not proven done, so a one-shot run must exit non-zero and
            // say which gate refused.
            Some(Event::TurnBlocked {
                gate,
                reason,
                manifest_path,
            }) => {
                if let Some(path) = manifest_path {
                    eprintln!("[evidence] {path}");
                }
                anyhow::bail!("{gate} gate blocked the turn: {reason}");
            }
            // A refused run reports the detail as an `Error` and the
            // verdict as the `RunBlocked` that follows it, so exiting on
            // the first `Error` would make the blocked exit unreachable.
            // Give the verdict a bounded moment to arrive.
            Some(Event::Error(msg)) => return Err(settle(&mut ev, msg).await),
            // Terminal, and not a clean exit: say so distinctly so a
            // wrapping script can tell a refused run from a finished one.
            Some(Event::RunBlocked { reason }) => return Err(blocked(reason)),
            Some(Event::TranscriptTrimmed { .. }) => {}
            Some(Event::SessionTitle { .. }) => {}
            // The agent task ended without saying how. A panic, an early
            // return, or a dropped sender all land here, and none of them
            // is a finished turn: the only clean exit is `TurnCompleted`.
            None => anyhow::bail!(
                "the agent stopped without a verdict: the event channel closed before any turn \
                 finished, so nothing was verified"
            ),
        }
    }
}

/// Decide what a fatal error exits as. Only a `RunBlocked` changes the
/// answer — an unrelated error still fails with its own message, and the
/// wait ends at the deadline, the channel closing, or the verdict,
/// whichever comes first.
async fn settle<E: Events>(ev: &mut E, detail: String) -> anyhow::Error {
    let deadline = Instant::now() + BLOCKED_VERDICT_GRACE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, ev.next()).await {
            Ok(Some(Event::RunBlocked { reason })) => return blocked(reason),
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => break,
        }
    }
    anyhow::anyhow!(detail)
}

fn blocked(reason: String) -> anyhow::Error {
    anyhow::anyhow!("run blocked: {reason}")
}

#[cfg(test)]
mod tests;
