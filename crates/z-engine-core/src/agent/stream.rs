//! Provider stream consumption: forwards text deltas to the UI, absorbs
//! tool-call deltas, records usage, honors Abort mid-stream.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use harness_provider::{ProviderError, StreamEvent, ToolCallAccumulator, Usage};

use super::events::{Command, Event};

pub(super) enum StreamOutcome {
    Completed,
    Aborted,
    Failed(ProviderError),
}

/// Consume provider events, forwarding text deltas to the UI, until Done /
/// error / abort. Watches the command channel concurrently so Abort is
/// honored mid-stream.
pub(super) async fn consume_stream(
    stream: &mut tokio::sync::mpsc::Receiver<Result<StreamEvent, ProviderError>>,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    text: &mut String,
    acc: &mut ToolCallAccumulator,
    usage_out: &mut Usage,
    abort_flag: &Arc<AtomicBool>,
) -> StreamOutcome {
    loop {
        tokio::select! {
            item = stream.recv() => {
                match item {
                    None => break,
                    Some(Err(e)) => return StreamOutcome::Failed(e),
                    Some(Ok(ev)) => match ev {
                        StreamEvent::TextDelta(t) => {
                            text.push_str(&t);
                            let _ = ev_tx.send(Event::TokenDelta(t));
                        }
                        StreamEvent::ReasoningDelta(r) => {
                            let _ = ev_tx.send(Event::ReasoningDelta(r));
                        }
                        StreamEvent::ToolCallDelta { index, id, name, args_delta } => {
                            acc.absorb(index, id.as_deref(), name.as_deref(), &args_delta);
                        }
                        StreamEvent::Usage(u) => {
                            // Latest prompt size + running completion total —
                            // the budget-pressure signal for v0.3's compactor.
                            // Replace (not max): after compaction the true
                            // prompt shrinks, and keeping the stale larger
                            // value would spuriously re-trigger compaction.
                            usage_out.prompt_tokens = u.prompt_tokens;
                            usage_out.completion_tokens =
                                usage_out.completion_tokens.saturating_add(u.completion_tokens);
                            let _ = ev_tx.send(Event::UsageUpdated {
                                prompt_tokens: usage_out.prompt_tokens,
                                completion_tokens: usage_out.completion_tokens,
                            });
                        }
                        // Non-terminal: usage may still arrive in later
                        // chunks (or already did in this batch).
                        StreamEvent::Finish(_) => {}
                        StreamEvent::Done => break,
                    },
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    None => {
                        abort_flag.store(true, Ordering::Relaxed);
                        return StreamOutcome::Aborted;
                    }
                    Some(Command::Abort) => {
                        abort_flag.store(true, Ordering::Relaxed);
                        return StreamOutcome::Aborted;
                    }
                    Some(_) => {} // approvals/submits are meaningless mid-stream
                }
            }
        }
    }
    StreamOutcome::Completed
}
