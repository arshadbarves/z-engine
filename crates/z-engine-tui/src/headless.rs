//! One-shot headless runner: submit a task, stream plain-text events,
//! exit non-zero on failure. Acceptance/CI companion to the TUI.

use z_engine_core::agent::{AgentHandle, ApprovalDecision, Event, EventRx};

pub async fn run_one_shot(
    handle: AgentHandle,
    mut ev: EventRx,
    task: &str,
    auto_approve: bool,
) -> anyhow::Result<()> {
    eprintln!("zengine --headless · task: {task}");
    handle.submit(task.to_string());

    loop {
        match ev.recv().await {
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
                    handle.approve(id, ApprovalDecision::Once);
                } else {
                    eprintln!(
                        "[denied: non-interactive] {tool} {input_preview}\n\
                         hint: rerun with --auto-approve to allow gated tools"
                    );
                    handle.deny(id);
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
            Some(Event::Error(msg)) => {
                anyhow::bail!(msg);
            }
            // Terminal, and not a clean exit: say so distinctly so a
            // wrapping script can tell a refused run from a finished one.
            Some(Event::RunBlocked { reason }) => {
                anyhow::bail!("run blocked: {reason}");
            }
            Some(Event::TranscriptTrimmed { .. }) => {}
            Some(Event::SessionTitle { .. }) => {}
            None => return Ok(()),
        }
    }
}
