use z_engine_core::agent::{Event, LoopConfig, spawn};

use crate::mock_loop::{Script, done, finish_json, serve, text_delta, tool_call_delta, wait_for};

#[tokio::test]
async fn subagent_exploration_stays_out_of_parent_context() {
    let tmp = tempfile::tempdir().unwrap();
    // The "broad exploration" artifact: a file whose NAME carries noise that
    // must never reach the parent transcript.
    std::fs::write(
        tmp.path().join("INTERMEDIATE_NOISE_42.rs"),
        "// nothing important\n",
    )
    .unwrap();

    let script = Script::default();
    // Parent round 1: delegate to a sub-agent.
    script.push(format!(
        "{}{}{}{}",
        text_delta("delegating"),
        tool_call_delta(
            0,
            Some("t1"),
            Some("task"),
            r#"{"prompt":"list rust files","max_tool_rounds":4}"#
        ),
        finish_json("tool_calls", 10, 10),
        done()
    ));
    // Parent round 2 (after sub finished): sees only the summary.
    script.push(format!(
        "{}{}{}",
        text_delta("SUMMARY_MARKER reached parent"),
        finish_json("stop", 20, 20),
        done()
    ));

    let base = serve(script.clone()).await;
    let cfg = LoopConfig {
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
    let (handle, mut ev) = spawn(cfg);
    handle.submit("explore broadly");

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));

    let bodies = script.requests_snapshot();

    // Classify requests.
    let sub_bodies: Vec<&String> = bodies
        .iter()
        .filter(|b| b.contains("research sub-agent"))
        .collect();
    let parent_bodies: Vec<&String> = bodies
        .iter()
        .filter(|b| !b.contains("research sub-agent"))
        .collect();
    assert_eq!(sub_bodies.len(), 2, "sub ran two rounds");
    assert_eq!(parent_bodies.len(), 2, "parent ran two rounds");

    // The sub saw the noisy filename (its own glob result).
    assert!(
        sub_bodies
            .iter()
            .any(|b| b.contains("INTERMEDIATE_NOISE_42")),
        "sub should have seen its exploration results"
    );

    // The parent's follow-up request carries the summary…
    let parent_round2 = parent_bodies[1];
    assert!(
        parent_bodies[0].contains("\"task\"") || parent_bodies[0].contains("task"),
        "sanity"
    );
    assert!(
        parent_round2.contains("SUB_SUMMARY_FACTS"),
        "summary missing from parent context"
    );
    // …but none of the sub's intermediate exploration noise.
    assert!(
        !parent_bodies
            .iter()
            .any(|b| b.contains("INTERMEDIATE_NOISE_42")),
        "intermediate exploration leaked into parent context"
    );

    // Token-delta measurement (spec acceptance): what the delegation ADDED
    // to the parent's context (round-2 minus round-1 request size) must be
    // far smaller than everything the sub-agent burned internally.
    let sub_bytes: usize = sub_bodies.iter().map(|b| b.len()).sum();
    let parent_delta = parent_round2.len().saturating_sub(parent_bodies[0].len());
    assert!(
        parent_delta < sub_bytes,
        "parent grew by {parent_delta}B but sub-agent burned {sub_bytes}B"
    );
    assert!(
        parent_delta < 1000,
        "summary bloated parent: {parent_delta}B"
    );
}
