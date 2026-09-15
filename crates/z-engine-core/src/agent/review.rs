//! Advisory review has explicit failure and cancellation outcomes. Neither a
//! transport failure nor a verdict mentioning the clean marker is a clean review.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use z_engine_provider::{ChatMessage, ChatRequest, Client};

use super::auxiliary::{AuxiliaryError, request_text};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReviewOutcome {
    NoFindings,
    Findings(String),
    Cancelled,
    Unavailable(String),
}

pub(super) async fn run_review(
    client: &Client,
    model: &str,
    task: &str,
    edit_results: &[String],
    abort: &Arc<AtomicBool>,
) -> ReviewOutcome {
    if abort.load(Ordering::Relaxed) {
        return ReviewOutcome::Cancelled;
    }
    let edits: Vec<_> = edit_results
        .iter()
        .map(|entry| {
            serde_json::json!({
                "diff": entry.chars().take(12_000).collect::<String>(),
                "truncated": entry.chars().count() > 12_000,
            })
        })
        .collect();
    let req = ChatRequest::new(
        model.to_string(),
        vec![
            ChatMessage::system(crate::prompts::REVIEWER),
            ChatMessage::user(serde_json::json!({"goal": task, "edits": edits}).to_string()),
        ],
    )
    .with_max_tokens(4096);
    match request_text(client, &req, abort).await {
        Ok(output) => classify(&output),
        Err(AuxiliaryError::Cancelled) => ReviewOutcome::Cancelled,
        Err(error) => ReviewOutcome::Unavailable(error.to_string()),
    }
}

fn classify(output: &str) -> ReviewOutcome {
    match output.trim() {
        "" => ReviewOutcome::Unavailable("Reviewer returned an empty response.".into()),
        "NO_FINDINGS" => ReviewOutcome::NoFindings,
        findings => ReviewOutcome::Findings(findings.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_exact_clean_verdict_is_clean() {
        assert_eq!(classify(" NO_FINDINGS\n"), ReviewOutcome::NoFindings);
        assert!(matches!(classify(""), ReviewOutcome::Unavailable(_)));
        assert!(matches!(
            classify("NO_FINDINGS is incorrect: a requirement is missing."),
            ReviewOutcome::Findings(_)
        ));
    }

    #[tokio::test]
    async fn cancelled_review_never_starts_a_provider_request() {
        let client = Client::new("http://127.0.0.1:1", None).unwrap();
        let outcome = run_review(
            &client,
            "fixture",
            "goal",
            &[],
            &Arc::new(AtomicBool::new(true)),
        )
        .await;
        assert_eq!(outcome, ReviewOutcome::Cancelled);
    }
}
