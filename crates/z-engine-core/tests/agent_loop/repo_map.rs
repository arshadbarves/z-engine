use z_engine_core::agent::{ApprovalDecision, Event, LoopConfig, spawn};

use crate::mock_loop::{Script, done, finish_json, serve, text_delta, tool_call_delta, wait_for};

#[tokio::test]
async fn repo_map_answers_where_defined_without_grep() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("src/main.rs"),
        "mod zebra;\nfn main() { println!(\"{}\", zebra::zebra_fn()); }\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("src/zebra.rs"),
        "pub fn zebra_fn() -> u32 { 42 }\n",
    )
    .unwrap();

    let script = Script::default();
    // The model answers purely from the injected symbol map — no tool calls.
    script.push(format!(
        "{}{}{}",
        text_delta("zebra_fn lives in src/zebra.rs line 1"),
        finish_json("stop", 40, 40),
        done()
    ));

    let base = serve(script.clone()).await;
    let cfg = LoopConfig {
        model: "test-model".into(),
        base_url: base.clone(),
        api_key: None,
        project_root: tmp.path().to_path_buf(),
        tmp_dir: tmp.path().join("tmp-out"),
        initial_allow_rules: vec![],
        max_context_tokens: 100_000,
        max_output_tokens: 16_384,
        max_task_continuations: 0,
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        shell_path: None,
    };
    let (handle, mut ev) = spawn(cfg);
    handle.submit("where is zebra_fn defined?");

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));

    // Map was injected into the very first request.
    let bodies = script.requests_snapshot();
    assert!(
        bodies[0].contains("src/zebra.rs:1 fn zebra_fn"),
        "repo map missing from request: {}",
        &bodies[0][..bodies[0].len().min(2000)]
    );

    // No tool spam: zero tool calls for this navigation question.
    assert_eq!(bodies.len(), 1);
}

#[tokio::test]
async fn repo_map_refreshes_after_edit() {
    let tmp = tempfile::tempdir().unwrap();

    let script = Script::default();
    // R1: model adds a new file via write_file (gated).
    script.push(format!(
        "{}{}{}{}",
        text_delta("adding"),
        tool_call_delta(
            0,
            Some("n1"),
            Some("write_file"),
            r#"{"path":"src/new_mod.rs","content":"pub fn giraffe_fn() {}\n"}"#
        ),
        finish_json("tool_calls", 9, 9),
        done()
    ));
    // R2: confirm — the refreshed map must now list giraffe_fn.
    script.push(format!(
        "{}{}{}",
        text_delta("added giraffe_fn"),
        finish_json("stop", 9, 9),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = LoopConfig {
        model: "test-model".into(),
        base_url: base,
        api_key: None,
        project_root: tmp.path().to_path_buf(),
        tmp_dir: tmp.path().join("tmp-out"),
        initial_allow_rules: vec![],
        max_context_tokens: 100_000,
        max_output_tokens: 16_384,
        max_task_continuations: 0,
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        shell_path: None,
    };
    cfg.initial_allow_rules.clear();
    let (handle, mut ev) = spawn(cfg);
    handle.submit("add giraffe module");

    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired { id, .. } = approval else {
        unreachable!()
    };
    handle.approve(id, ApprovalDecision::Once);

    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;

    // Request #2 must include the refreshed map containing the new symbol.
    let bodies = script.requests_snapshot();
    assert!(bodies.len() >= 2);
    assert!(
        bodies[1].contains("new_mod.rs"),
        "refreshed map missing from second request"
    );
}
