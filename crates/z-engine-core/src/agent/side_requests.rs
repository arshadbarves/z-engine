//! Auxiliary model calls for compaction and session titles.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use z_engine_provider::{ChatMessage, ChatRequest, Client};

use super::LoopConfig;
use super::auxiliary::{AuxiliaryError, request_text};

/// Side-request that compresses demoted turns into terse summary bullets.
pub(super) async fn summarize_segment(
    client: &Client,
    cfg: &LoopConfig,
    input: &str,
    abort: &Arc<AtomicBool>,
) -> Result<String, AuxiliaryError> {
    let clipped: String = input
        .chars()
        .take(crate::context::compact::MAX_SUMMARIZE_CHARS)
        .collect();
    let req = ChatRequest::new(
        cfg.model.clone(),
        vec![
            ChatMessage::system(crate::prompts::SUMMARIZER),
            ChatMessage::user(clipped),
        ],
    )
    .with_max_tokens(2048);
    request_text(client, &req, abort).await
}

/// Non-blocking title for the sessions sidebar. Failures return `None`
/// so the caller can fall back to a clipped first line of the prompt.
pub(super) async fn generate_session_title(
    client: &Client,
    model: &str,
    prompt: &str,
) -> Option<String> {
    let clipped: String = prompt.chars().take(800).collect();
    let req = ChatRequest::new(
        model.to_string(),
        vec![
            ChatMessage::system(crate::prompts::SESSION_TITLE),
            ChatMessage::user(clipped),
        ],
    )
    .with_max_tokens(128);
    let abort = Arc::new(AtomicBool::new(false));
    match request_text(client, &req, &abort).await {
        Ok(output) => sanitize_session_title(&output),
        Err(error) => {
            tracing::warn!(%error, "session-title request failed");
            None
        }
    }
}

/// First line, strip wrapping quotes, at most 8 words. Empty → None.
pub(super) fn sanitize_session_title(raw: &str) -> Option<String> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let stripped = line.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    let words: Vec<&str> = stripped.split_whitespace().take(8).collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_session_title;

    #[test]
    fn strips_quotes_and_caps_words() {
        assert_eq!(
            sanitize_session_title("\"Fix the flaky auth test in login.rs extra words here\""),
            Some("Fix the flaky auth test in login.rs extra".into())
        );
    }

    #[test]
    fn empty_raw_is_none() {
        assert_eq!(sanitize_session_title("  \n  "), None);
    }
}
