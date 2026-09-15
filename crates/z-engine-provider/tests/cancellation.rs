//! Cancellation must interrupt a request even when the provider sends no data.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use z_engine_provider::{ChatMessage, ChatRequest, Client, StreamEvent};

async fn stalled_provider(
    sse: bool,
) -> (
    Client,
    tokio::sync::oneshot::Receiver<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0u8; 8192];
        assert!(socket.read(&mut bytes).await.unwrap() > 0);
        if sse {
            let event =
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"started\"}}]}\n\n";
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
            socket
                .write_all(format!("{:x}\r\n{event}\r\n", event.len()).as_bytes())
                .await
                .unwrap();
        }
        started.send(()).unwrap();
        loop {
            match socket.read(&mut bytes).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                    ) =>
                {
                    break;
                }
                Err(error) => panic!("unexpected socket error: {error}"),
            }
        }
    });
    (
        Client::new(&format!("http://{address}"), None).unwrap(),
        ready,
        server,
    )
}

async fn cancellation_closes_connection(sse: bool, drop_receiver: bool) {
    let (client, ready, server) = stalled_provider(sse).await;
    let abort = Arc::new(AtomicBool::new(false));
    let request = ChatRequest::new("fixture", vec![ChatMessage::user("test")]);
    let mut receiver = client.stream_chat(&request, Arc::clone(&abort));
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    if sse {
        let event = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(event, StreamEvent::TextDelta(_)));
    }
    if drop_receiver {
        drop(receiver);
    } else {
        abort.store(true, Ordering::Relaxed);
        assert!(
            tokio::time::timeout(Duration::from_secs(2), receiver.recv())
                .await
                .unwrap()
                .is_none()
        );
    }
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn dropping_receiver_cancels_pending_response_headers() {
    cancellation_closes_connection(false, true).await;
}

#[tokio::test]
async fn abort_cancels_pending_response_headers() {
    cancellation_closes_connection(false, false).await;
}

#[tokio::test]
async fn dropping_receiver_cancels_stalled_sse_read() {
    cancellation_closes_connection(true, true).await;
}

#[tokio::test]
async fn abort_cancels_stalled_sse_read() {
    cancellation_closes_connection(true, false).await;
}
