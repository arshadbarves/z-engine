//! OpenAI-compatible HTTP client.
//!
//! One adapter serves every compatible backend (OpenRouter, OpenAI,
//! Ollama, LM Studio, Groq, …). Streaming uses SSE (`stream: true`) and
//! surfaces typed [`StreamEvent`]s over a channel so the agent loop can
//! `select!` against user commands (abort etc.) while consuming tokens.
//!
//! Retry policy (v0.1 scope):
//! - connection errors / timeouts / `429` / `5xx` before the stream starts:
//!   exponential backoff (250 ms · 2ⁿ, capped 8 s), honoring `Retry-After`,
//!   up to [`MAX_ATTEMPTS`] tries;
//! - other `4xx`: immediate failure;
//! - an established stream that dies mid-flight: surfaced immediately as
//!   [`ProviderError::StreamInterrupted`] (no silent auto-replay yet).
//!
//! The API key lives only in the `Authorization` header and is never
//! logged; `Debug` renders it as `<redacted>`.

use super::sse::SseDecoder;
use super::types::{ChatRequest, StreamEvent};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Initial + retries before giving up on establishing a stream.
const MAX_ATTEMPTS: u32 = 4;
const BASE_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_millis(8_000);
const ERROR_BODY_SNIPPET: usize = 2_000;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider misconfigured: {0}")]
    Config(String),
    #[error("failed building request: {0}")]
    RequestBuild(String),
    #[error("provider returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("rate limited ({status}) after {attempts} attempts; {detail}")]
    RateLimited {
        status: u16,
        attempts: u32,
        detail: String,
    },
    #[error("connection failed after {attempts} attempts: {cause}")]
    Connect { attempts: u32, cause: String },
    #[error("stream interrupted: {0}")]
    StreamInterrupted(String),
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Client {
    /// Build a client. `api_key` comes exclusively from `HARNESS_API_KEY`
    /// upstream; local servers (Ollama/LM Studio) may run without one.
    pub fn new(base_url: &str, api_key: Option<String>) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ProviderError::Config(format!("http client: {e}")))?;
        let mut base = base_url.trim().to_string();
        while base.ends_with('/') {
            base.pop();
        }
        if base.is_empty() {
            return Err(ProviderError::Config("base_url is empty".into()));
        }
        Ok(Self {
            http,
            base_url: base,
            api_key,
        })
    }

    /// Start a streaming chat completion.
    ///
    /// Returns a receiver of events ending with either `Done`-adjacent
    /// normal closure or a single terminal `Err`. The `abort` flag is
    /// checked between every chunk for instant cancellation.
    pub fn stream_chat(
        &self,
        req: &ChatRequest,
        abort: Arc<std::sync::atomic::AtomicBool>,
    ) -> mpsc::Receiver<Result<StreamEvent, ProviderError>> {
        let (tx, rx) = mpsc::channel::<Result<StreamEvent, ProviderError>>(64);
        let http = self.http.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();

        let body = match serde_json::to_vec(req) {
            Ok(b) => b,
            Err(e) => {
                let tx_err = tx;
                // Channel dropped instantly is fine; surface synchronously
                // through the same channel contract by spawning anyway.
                tokio::spawn(async move {
                    let _ = tx_err
                        .send(Err(ProviderError::RequestBuild(e.to_string())))
                        .await;
                });
                return rx;
            }
        };

        tokio::spawn(async move {
            let mut attempt: u32 = 0;
            let response = loop {
                attempt += 1;
                if abort.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                let mut request = http
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(body.clone());
                if let Some(key) = &api_key {
                    request = request.bearer_auth(key);
                }

                match request.send().await {
                    Ok(resp) if resp.status().is_success() => break Ok(resp),
                    Ok(resp) => {
                        let status = resp.status();
                        let retry_after = retry_after_of(&resp);
                        let snippet = resp
                            .text()
                            .await
                            .unwrap_or_default()
                            .chars()
                            .take(ERROR_BODY_SNIPPET)
                            .collect::<String>();
                        let retryable = status.as_u16() == 429 || status.is_server_error();
                        if !retryable {
                            tracing::warn!(%status, len = snippet.len(), "non-retryable provider error");
                            break Err(ProviderError::Http {
                                status: status.as_u16(),
                                body: snippet,
                            });
                        }
                        if attempt >= MAX_ATTEMPTS {
                            if status.as_u16() == 429 {
                                break Err(ProviderError::RateLimited {
                                    status: status.as_u16(),
                                    attempts: attempt,
                                    detail: rate_limit_detail(&snippet, retry_after),
                                });
                            }
                            break Err(ProviderError::Http {
                                status: status.as_u16(),
                                body: snippet,
                            });
                        }
                        let delay = retry_after.unwrap_or_else(|| backoff_delay(attempt));
                        tracing::info!(%status, ?delay, attempt, "retryable provider error; backing off");
                        tokio::time::sleep(delay).await;
                    }
                    Err(e) => {
                        let retryable = e.is_connect() || e.is_timeout();
                        if !retryable || attempt >= MAX_ATTEMPTS {
                            break Err(ProviderError::Connect {
                                attempts: attempt,
                                cause: e.to_string(),
                            });
                        }
                        let delay = backoff_delay(attempt);
                        tracing::info!(attempt, ?delay, error = %e, "transport error; retrying");
                        tokio::time::sleep(delay).await;
                    }
                }
            };

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };

            let mut decoder = SseDecoder::new();
            use futures::StreamExt;
            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await {
                if abort.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::debug!("provider stream aborted");
                    return;
                }
                match item {
                    Ok(bytes) => {
                        for ev in decoder.feed(&bytes) {
                            if tx.send(Ok(ev)).await.is_err() {
                                return; // consumer gone
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(ProviderError::StreamInterrupted(e.to_string())))
                            .await;
                        return;
                    }
                }
            }
            for ev in decoder.finish() {
                let _ = tx.send(Ok(ev)).await;
            }
        });

        rx
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    BASE_BACKOFF
        .saturating_mul(2u32.saturating_pow(attempt.saturating_sub(1)))
        .min(MAX_BACKOFF)
}

fn retry_after_of(resp: &reqwest::Response) -> Option<Duration> {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(|secs| Duration::from_secs(secs).min(MAX_BACKOFF))
}

fn rate_limit_detail(body_snippet: &str, retry_after: Option<Duration>) -> String {
    match retry_after {
        Some(d) => format!("retry advised after {d:?}; provider said: {body_snippet}"),
        None => format!("provider said: {body_snippet}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_api_key() {
        let c = Client::new("https://example.invalid/v1", Some("sk-super-secret".into())).unwrap();
        let rendered = format!("{c:?}");
        assert!(!rendered.contains("sk-super-secret"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn trailing_slashes_trimmed_empty_rejected() {
        let c = Client::new("https://example.invalid/v1///", None).unwrap();
        assert_eq!(c.base_url, "https://example.invalid/v1");
        assert!(Client::new("   ", None).is_err());
    }

    #[test]
    fn backoff_is_exponential_then_capped() {
        assert_eq!(backoff_delay(1), Duration::from_millis(250));
        assert_eq!(backoff_delay(2), Duration::from_millis(500));
        assert_eq!(backoff_delay(3), Duration::from_millis(1_000));
        assert_eq!(backoff_delay(9), MAX_BACKOFF);
    }

    /// Connection-refused must actually be retried with backoff (spec §10),
    /// not fail fast. Regression guard for the retry classifier.
    #[tokio::test]
    async fn connect_refused_backs_off_then_errors() {
        use crate::{ChatMessage, ChatRequest};
        let c = Client::new("http://127.0.0.1:9", Some("k".into())).unwrap();
        let req = ChatRequest::new("m", vec![ChatMessage::user("hi")]);
        let abort = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut rx = c.stream_chat(&req, abort);
        let started = std::time::Instant::now();
        let mut last = None;
        while let Some(item) = rx.recv().await {
            if let Err(e) = item {
                last = Some(e);
            }
        }
        let err = last.expect("expected terminal error");
        // 3 backoffs: 250+500+1000ms minimum across 4 attempts
        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(1500),
            "no backoff ({elapsed:?}); err={err}"
        );
        assert!(
            matches!(err, ProviderError::Connect { attempts: 4, .. }),
            "{err}"
        );
    }
}
