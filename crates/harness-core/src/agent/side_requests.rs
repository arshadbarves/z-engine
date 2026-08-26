//! Side requests: model calls outside the main loop (post-edit reviewer,
//! compaction summarizer). Failures never block the turn.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use harness_provider::{ChatMessage, ChatRequest, Client, StreamEvent};

use super::LoopConfig;

/// Post-edit reviewer (spec section 9 v0.9): a side-request that audits
/// this round's diffs against the original task. Returns findings text, or
/// None for "no findings" / transport failure (never blocks the turn).
pub(super) async fn run_review(
    client: &Client,
    model: &str,
    task: &str,
    edit_results: &[String],
) -> Option<String> {
    let mut body = String::from("# Original task\n");
    body.push_str(task.trim());
    body.push_str("\n\n# Edits applied this round\n");
    for (i, entry) in edit_results.iter().enumerate() {
        let clipped: String = entry.chars().take(3_000).collect();
        let _ =
            std::fmt::Write::write_fmt(&mut body, format_args!("\n## Edit {}\n{clipped}\n", i + 1));
    }

    let req = ChatRequest::new(
        model.to_string(),
        vec![
            ChatMessage::system(crate::prompts::REVIEWER),
            ChatMessage::user(body),
        ],
    );
    let abort = Arc::new(AtomicBool::new(false));
    let mut rx = client.stream_chat(&req, abort);
    let mut out = String::new();
    while let Some(item) = rx.recv().await {
        match item {
            Ok(StreamEvent::TextDelta(t)) => out.push_str(&t),
            Ok(StreamEvent::Done) | Ok(StreamEvent::Finish(_)) => {}
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "reviewer stream failed");
                return None;
            }
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() || out.contains("NO_FINDINGS") {
        None
    } else {
        Some(out)
    }
}

/// Side-request that compresses demoted turns into terse summary bullets.
pub(super) async fn summarize_segment(client: &Client, cfg: &LoopConfig, input: &str) -> String {
    let clipped: String = input.chars().take(12_000).collect();
    let req = ChatRequest::new(
        cfg.model.clone(),
        vec![
            ChatMessage::system(crate::prompts::SUMMARIZER),
            ChatMessage::user(clipped),
        ],
    );
    let abort = Arc::new(AtomicBool::new(false));
    let mut rx = client.stream_chat(&req, abort);
    let mut out = String::new();
    while let Some(item) = rx.recv().await {
        match item {
            Ok(StreamEvent::TextDelta(t)) => out.push_str(&t),
            Ok(StreamEvent::Done) | Ok(StreamEvent::Finish(_)) => {}
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "summarizer stream failed");
                return String::new();
            }
        }
    }
    out.trim().to_string()
}
