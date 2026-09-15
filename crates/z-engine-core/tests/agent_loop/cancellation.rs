use z_engine_core::agent::{Event, spawn};

use crate::mock_loop::{Script, cfg_for, done, finish_json, serve, text_delta, wait_for};

#[tokio::test]
async fn abort_mid_stream_ends_turn_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // A very long single response: enough drips to still be streaming.
    let long_body =
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"tick\"}}]}\n\n".repeat(200_000);
    script.push(long_body);

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("count ticks");

    let _ = wait_for(&mut ev, |e| matches!(e, Event::TokenDelta(_))).await;
    handle.abort();

    let aborted = wait_for(&mut ev, |e| matches!(e, Event::TurnAborted)).await;
    assert!(matches!(aborted, Event::TurnAborted));
}

#[tokio::test]
async fn shutdown_stops_the_task() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        text_delta("hi"),
        finish_json("stop", 1, 1),
        done()
    ));
    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.shutdown();
    // Once the task exits, the event channel closes (recv → None forever).
    while ev.recv().await.is_some() {}
}
