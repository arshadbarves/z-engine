//! One-shot headless runner: submit a task, stream plain-text events,
//! exit non-zero on failure. Acceptance/CI companion to the TUI.

use harness_core::agent::{AgentHandle, ApprovalDecision, Event, EventRx};

pub async fn run_one_shot(
    handle: AgentHandle,
    mut ev: EventRx,
    task: &str,
    auto_approve: bool,
) -> anyhow::Result<()> {
    eprintln!("harness --headless · task: {task}");
    handle.submit(task.to_string());

    loop {
        match ev.recv().await {
            Some(Event::TokenDelta(t)) => {
                use std::io::Write;
                print!("{t}");
                std::io::stdout().flush().ok();
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
            None => return Ok(()),
        }
    }
}
