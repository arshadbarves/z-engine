use z_engine_core::agent::{Event, spawn};

use crate::mock_loop::{
    Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for,
};

#[tokio::test]
async fn mcp_echo_tool_roundtrips() {
    use z_engine_core::config::Config;

    let tmp = tempfile::tempdir().unwrap();
    // Project config registers the echo server (tests project layering too).
    std::fs::create_dir_all(tmp.path().join(".z-engine")).unwrap();
    std::fs::write(
        tmp.path().join(".z-engine/config.toml"),
        "[mcp.servers.echo]\ncommand = \"python3\"\nargs = [\"scripts/mcp_echo_server.py\"]\n",
    )
    .unwrap();

    let loaded = Config::load(Some(tmp.path())).unwrap();
    assert_eq!(loaded.mcp_servers.len(), 1);
    // Rewrite the relative script path to an absolute one.
    let mut srv = loaded.mcp_servers[0].clone();
    srv.args = vec![format!(
        "{}/../../scripts/mcp_echo_server.py",
        env!("CARGO_MANIFEST_DIR")
    )];

    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("calling echo"),
        tool_call_delta(0, Some("m1"), Some("echo"), r#"{"text":"ping-marker"}"#),
        finish_json("tool_calls", 5, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("got pong."),
        finish_json("stop", 8, 8),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base, tmp.path());
    cfg.mcp_servers = vec![srv];
    cfg.auto_allow_tools = vec!["echo".into()];
    let (handle, mut ev) = spawn(cfg);
    handle.submit("use echo");

    let _ = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "echo"),
    )
    .await;
    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));

    let bodies = script.requests_snapshot();
    assert!(
        bodies.iter().any(|b| b.contains("PONG:ping-marker")),
        "echo result never reached the model"
    );
}
