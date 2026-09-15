//! Bounded text-only side requests. Dropping their receiver cancels transport.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc::Receiver;
use z_engine_provider::{ChatRequest, Client, ProviderError, StreamEvent};

#[derive(Debug, thiserror::Error)]
pub(super) enum AuxiliaryError {
    #[error("auxiliary model request cancelled")]
    Cancelled,
    #[error("auxiliary model request exceeded its deadline")]
    Timeout,
    #[error("auxiliary model output exceeded its byte limit")]
    OutputLimit,
    #[error("auxiliary model returned an empty response")]
    Empty,
    #[error("auxiliary model request failed: {0}")]
    Provider(#[from] ProviderError),
}

pub(super) async fn request_text(
    client: &Client,
    request: &ChatRequest,
    abort: &Arc<AtomicBool>,
) -> Result<String, AuxiliaryError> {
    if abort.load(Ordering::Relaxed) {
        return Err(AuxiliaryError::Cancelled);
    }
    collect(
        client.stream_chat(request, Arc::clone(abort)),
        abort,
        Duration::from_secs(60),
        64 * 1024,
    )
    .await
}

async fn collect(
    mut stream: Receiver<Result<StreamEvent, ProviderError>>,
    abort: &AtomicBool,
    timeout: Duration,
    max_bytes: usize,
) -> Result<String, AuxiliaryError> {
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut cancellation = tokio::time::interval(Duration::from_millis(50));
    let mut output = String::new();
    loop {
        if abort.load(Ordering::Relaxed) {
            return Err(AuxiliaryError::Cancelled);
        }
        tokio::select! {
            _ = &mut deadline => return Err(AuxiliaryError::Timeout),
            _ = cancellation.tick() => {}
            item = stream.recv() => match item {
                Some(Ok(StreamEvent::TextDelta(text))) => {
                    if output.len().saturating_add(text.len()) > max_bytes {
                        return Err(AuxiliaryError::OutputLimit);
                    }
                    output.push_str(&text);
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error.into()),
                None => {
                    if abort.load(Ordering::Relaxed) {
                        return Err(AuxiliaryError::Cancelled);
                    }
                    return if output.trim().is_empty() {
                        Err(AuxiliaryError::Empty)
                    } else {
                        Ok(output.trim().to_owned())
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::channel;

    #[tokio::test]
    async fn stalled_stream_times_out_and_closes_its_receiver() {
        let (sender, receiver) = channel(4);
        let error = collect(
            receiver,
            &AtomicBool::new(false),
            Duration::from_millis(10),
            64,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AuxiliaryError::Timeout));
        assert!(sender.is_closed());
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_open_stream() {
        let (sender, receiver) = channel(4);
        let abort = AtomicBool::new(false);
        let request = collect(receiver, &abort, Duration::from_secs(1), 64);
        let cancel = async {
            tokio::task::yield_now().await;
            abort.store(true, Ordering::Relaxed);
        };
        let (result, ()) = tokio::join!(request, cancel);
        assert!(matches!(result, Err(AuxiliaryError::Cancelled)));
        assert!(sender.is_closed());
    }

    #[tokio::test]
    async fn output_limit_and_empty_stream_are_explicit_errors() {
        let (sender, receiver) = channel(4);
        sender
            .send(Ok(StreamEvent::TextDelta("oversized".into())))
            .await
            .unwrap();
        assert!(matches!(
            collect(receiver, &AtomicBool::new(false), Duration::from_secs(1), 4).await,
            Err(AuxiliaryError::OutputLimit)
        ));
        assert!(sender.is_closed());
        let (sender, receiver) = channel(4);
        drop(sender);
        assert!(matches!(
            collect(receiver, &AtomicBool::new(false), Duration::from_secs(1), 4).await,
            Err(AuxiliaryError::Empty)
        ));
    }
}
