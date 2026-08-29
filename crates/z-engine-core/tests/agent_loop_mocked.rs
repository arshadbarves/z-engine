//! Integration tests: the full agent loop against a mocked
//! OpenAI-compatible provider serving canned SSE (spec §10).

mod common;

use std::time::Duration;

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for};
use z_engine_core::agent::{ApprovalDecision, Event, LoopConfig, spawn};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_loop_read_then_bash_then_answer() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("notes.txt"), "the secret number is 41\n").unwrap();

    let script = Script::default();
    // R1: read a file · R2: allowed bash · R3: final answer.
    script.push(format!(
        "{}{}{}{}",
        text_delta("Let me look."),
        tool_call_delta(
            0,
            Some("call_read"),
            Some("read_file"),
            r#"{"path":"notes.txt"}"#
        ),
        finish_json("tool_calls", 100, 20),
        done()
    ));
    script.push(format!(
        "{}{}{}{}",
        text_delta("Now incrementing."),
        tool_call_delta(
            0,
            Some("call_bash"),
            Some("bash"),
            r#"{"command":"echo 42 > out.txt"}"#
        ),
        finish_json("tool_calls", 200, 40),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("Done — wrote 42."),
        finish_json("stop", 300, 60),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("bump the number");

    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnStarted)).await;

    let finished = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "read_file"),
    )
    .await;
    let Event::ToolCallFinished { ok, summary, .. } = finished else {
        unreachable!()
    };
    assert!(ok, "{summary}");
    assert!(summary.contains("notes.txt"));

    // `echo*` was pre-allowed → bash runs without any ApprovalRequired.
    let bash_done = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "bash"),
    )
    .await;
    assert!(matches!(
        bash_done,
        Event::ToolCallFinished { ok: true, .. }
    ));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    let Event::TurnCompleted {
        prompt_tokens,
        completion_tokens,
    } = completed
    else {
        unreachable!()
    };
    assert_eq!(prompt_tokens, 300); // latest prompt size wins
    assert_eq!(completion_tokens, 120); // 20 + 40 + 60 cumulative

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("out.txt"))
            .unwrap()
            .trim(),
        "42"
    );

    assert_eq!(script.request_count(), 3);
}

#[tokio::test]
async fn tool_result_is_fed_back_to_the_model() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("fact.txt"), "harness-was-here\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("reading"),
        tool_call_delta(0, Some("cr"), Some("read_file"), r#"{"path":"fact.txt"}"#),
        finish_json("tool_calls", 10, 10),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("got it"),
        finish_json("stop", 20, 20),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("read fact");

    let _ = wait_for(&mut ev, |e| {
        matches!(e, Event::ToolCallFinished { ok: true, .. })
    })
    .await;
    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;

    // The second POST must carry the tool result back to the model.
    for _ in 0..50 {
        if script.request_count() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let reqs = script.requests_snapshot();
    assert!(
        reqs.len() >= 2 && reqs[1].contains("harness-was-here"),
        "tool result should be fed back in request #2"
    );
}

#[tokio::test]
async fn gated_bash_prompt_then_deny_refuses_and_model_adapts() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("rm time"),
        tool_call_delta(
            0,
            Some("c1"),
            Some("bash"),
            r#"{"command":"rm dangerous-thing"}"#
        ),
        finish_json("tool_calls", 5, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("Understood, adjusting."),
        finish_json("stop", 6, 6),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("clean up");

    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired {
        id,
        tool,
        input_preview,
        suggested_rule,
        ..
    } = approval
    else {
        unreachable!()
    };
    assert_eq!(tool, "bash");
    assert!(input_preview.contains("dangerous"));
    assert_eq!(suggested_rule.as_deref(), Some("rm dangerous-thing*"));

    handle.deny(id);

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

#[tokio::test]
async fn approve_always_prefix_skips_second_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // Two consecutive identical gated commands, then a closing answer.
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("c0"),
            Some("bash"),
            r#"{"command":"cargo test all"}"#
        ),
        finish_json("tool_calls", 9, 9),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("c1"),
            Some("bash"),
            r#"{"command":"cargo test all"}"#
        ),
        finish_json("tool_calls", 9, 9),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("all green"),
        finish_json("stop", 9, 9),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("run tests");

    // First prompt → answer "always this prefix".
    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired {
        id, suggested_rule, ..
    } = approval
    else {
        unreachable!()
    };
    assert_eq!(suggested_rule.as_deref(), Some("cargo test*"));
    handle.approve(
        id,
        ApprovalDecision::AlwaysSession {
            rule: suggested_rule.unwrap(),
        },
    );

    // Second identical command must NOT re-prompt: straight to completion.
    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

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
async fn malformed_tool_arguments_become_error_result_not_crash() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // Arguments never form valid JSON.
    script.push(format!(
        "{}{}{}",
        tool_call_delta(0, Some("cbad"), Some("read_file"), "{\"path\": "),
        finish_json("tool_calls", 4, 4),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("recovered."),
        finish_json("stop", 5, 5),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("go");

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
}

#[tokio::test]
async fn parallel_safe_tools_run_in_one_round() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "A\n").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "B\n").unwrap();

    let script = Script::default();
    // Two read_file calls in ONE assistant message (indexes 0 and 1).
    script.push(format!(
        "{}{}{}{}",
        tool_call_delta(0, Some("p1"), Some("read_file"), r#"{"path":"a.txt"}"#),
        tool_call_delta(1, Some("p2"), Some("read_file"), r#"{"path":"b.txt"}"#),
        finish_json("tool_calls", 7, 7),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("both read"),
        finish_json("stop", 8, 8),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("read both");

    let finished_a = wait_for(&mut ev, |e| matches!(e, Event::ToolCallStarted { .. })).await;
    assert!(matches!(finished_a, Event::ToolCallStarted { .. }));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
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

#[tokio::test]
async fn edit_file_flow_with_diff_preview_and_approval() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("cfg.txt"), "mode = dev\nlevel = 3\n").unwrap();

    let script = Script::default();
    // R1: read the file. R2: edit it (gated). R3: confirm.
    script.push(format!(
        "{}{}{}{}",
        text_delta("inspecting"),
        tool_call_delta(0, Some("e0"), Some("read_file"), r#"{"path":"cfg.txt"}"#),
        finish_json("tool_calls", 10, 10),
        done()
    ));
    script.push(format!(
        "{}{}{}{}",
        text_delta("editing now"),
        tool_call_delta(
            0,
            Some("e1"),
            Some("edit_file"),
            r#"{"path":"cfg.txt","old_string":"mode = dev","new_string":"mode = prod"}"#
        ),
        finish_json("tool_calls", 20, 20),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("promoted."),
        finish_json("stop", 30, 30),
        done()
    ));

    let base = serve(script.clone()).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("flip mode");

    let _ = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "read_file"),
    )
    .await;

    // The gated edit must carry a unified-diff preview.
    let approval = wait_for(
        &mut ev,
        |e| matches!(e, Event::ApprovalRequired { tool, .. } if tool == "edit_file"),
    )
    .await;
    let Event::ApprovalRequired {
        id,
        detail_preview,
        input_preview,
        ..
    } = approval
    else {
        unreachable!()
    };
    assert!(input_preview.contains("mode"));
    let diff = detail_preview.expect("edit_file should attach a diff preview");
    assert!(diff.contains("--- a/cfg.txt"), "{diff}");
    assert!(diff.contains("+mode = prod"), "{diff}");
    assert!(diff.contains("-mode = dev"), "{diff}");

    handle.approve(id, ApprovalDecision::Once);

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("cfg.txt")).unwrap(),
        "mode = prod\nlevel = 3\n"
    );
}

#[tokio::test]
async fn edit_without_prior_read_is_refused_then_model_reroutes() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "content\n").unwrap();

    let script = Script::default();
    // R1: model tries edit_file WITHOUT reading first -> error tool-result.
    script.push(format!(
        "{}{}{}{}",
        text_delta("quick edit"),
        tool_call_delta(
            0,
            Some("r1"),
            Some("edit_file"),
            r#"{"path":"f.txt","old_string":"content","new_string":"changed"}"#
        ),
        finish_json("tool_calls", 5, 5),
        done()
    ));
    // R2: model reads then answers.
    script.push(format!(
        "{}{}{}{}",
        text_delta("reading first"),
        tool_call_delta(0, Some("r2a"), Some("read_file"), r#"{"path":"f.txt"}"#),
        finish_json("tool_calls", 8, 8),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("understood"),
        finish_json("stop", 9, 9),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("edit f");

    // The gated edit prompts first; approving lets it run, and the
    // read-before-edit tracker then refuses inside the tool.
    let approval = wait_for(
        &mut ev,
        |e| matches!(e, Event::ApprovalRequired { tool, .. } if tool == "edit_file"),
    )
    .await;
    let Event::ApprovalRequired {
        id, detail_preview, ..
    } = approval
    else {
        unreachable!()
    };
    assert!(
        detail_preview.is_some(),
        "diff preview present even pre-refusal"
    );
    handle.approve(id, ApprovalDecision::Once);

    let refused = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, ok: false, .. } if name == "edit_file"),
    )
    .await;
    assert!(matches!(refused, Event::ToolCallFinished { ok: false, .. }));

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
    // File untouched by the refused attempt.
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("f.txt")).unwrap(),
        "content\n"
    );
}

#[tokio::test]
async fn long_session_compaction_preserves_coherence() {
    let tmp = tempfile::tempdir().unwrap();

    // A big fixture whose early lines carry a fact that must survive.
    let mut big = String::from("filler\nfiller\nTHE_SECRET_ZEBRA_GRAZES_AT_NOON\n");
    for i in 0..1500 {
        big.push_str(&format!(
            "line {i}: lorem ipsum dolor sit amet consectetur adipiscing elit\n"
        ));
    }
    std::fs::write(tmp.path().join("big.txt"), &big).unwrap();

    let script = Script::default();
    // Three heavy read rounds with climbing usage; budget is 100k so the
    // third crosses the 92% auto-compaction threshold.
    for (i, (id, prompt)) in [("h1", 30_000u64), ("h2", 55_000), ("h3", 95_000)]
        .into_iter()
        .enumerate()
    {
        let _ = i;
        script.push(format!(
            "{}{}{}{}",
            text_delta("reading more"),
            tool_call_delta(
                0,
                Some(id),
                Some("read_file"),
                r#"{"path":"big.txt","limit":2000}"#
            ),
            finish_json("tool_calls", prompt, 10),
            done()
        ));
    }
    // Final plain answer once compaction has happened.
    script.push(format!(
        "{}{}{}",
        text_delta("all done"),
        finish_json("stop", 99_999, 5),
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
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 4,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        guarded: false,
    };
    let (handle, mut ev) = spawn(cfg);
    handle.submit("ingest big file");

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    let Event::TurnCompleted { .. } = completed else {
        unreachable!()
    };

    // The summarizer side-request must have been served (mock answers it
    // with a summary carrying the secret fact).
    let saw_summarizer = {
        let bodies: Vec<String> = script
            .requests_snapshot()
            .into_iter()
            .filter(|b| b.contains("compress an earlier portion"))
            .collect();
        !bodies.is_empty()
    };
    assert!(saw_summarizer, "summarizer side-request never ran");

    let bodies = script.requests_snapshot();
    // Some request carried the L1 notes block with the summarized fact.
    assert!(
        bodies
            .iter()
            .any(|b| b.contains("THE_SECRET_ZEBRA_GRAZES_AT_NOON")
                && b.contains("Session context notes")),
        "summary fact never re-entered context"
    );
    // Elided markers appear once compaction trimmed old outputs.
    assert!(
        bodies.iter().any(|b| b.contains("[harness:elided")),
        "no elided tool outputs observed"
    );
    // Spill files preserve the full earlier outputs.
    let spills: Vec<_> = std::fs::read_dir(tmp.path().join("tmp-out/z-engine"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert!(
        spills.iter().any(|p| {
            p.file_name().unwrap().to_string_lossy().starts_with("ctx-")
                && std::fs::read_to_string(p).unwrap().contains("SECRET_ZEBRA")
        }),
        "spill file missing"
    );
}

#[tokio::test]
async fn always_persist_writes_config_and_never_reprompts() {
    use z_engine_core::config::{CliOverrides, Config, project_config_path};
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    // Two identical gated commands in separate rounds, then a close-out.
    for i in 0..2 {
        script.push(format!(
            "{}{}{}",
            tool_call_delta(
                0,
                Some(&format!("pc{i}")),
                Some("bash"),
                r#"{"command":"terraform plan"}"#
            ),
            finish_json(if i == 1 { "stop" } else { "tool_calls" }, 9, 9),
            done()
        ));
    }
    script.push(format!(
        "{}{}{}",
        text_delta("fin"),
        finish_json("stop", 9, 9),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base.clone(), tmp.path());
    cfg.initial_allow_rules.clear();
    let (handle, mut ev) = spawn(cfg);
    handle.submit("plan infra");

    // First prompt answered with AlwaysPersist.
    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired {
        id,
        suggested_rule,
        can_persist,
        detail_preview,
        ..
    } = approval
    else {
        unreachable!()
    };
    assert_eq!(suggested_rule.as_deref(), Some("terraform plan*"));
    assert!(can_persist, "in-root bash may persist");
    let _ = detail_preview;
    handle.approve(
        id,
        ApprovalDecision::AlwaysPersist {
            rule: suggested_rule.unwrap(),
        },
    );

    // Second identical command must not re-prompt.
    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));

    // Rule landed in the project config.
    let cfg_text = std::fs::read_to_string(project_config_path(tmp.path())).unwrap();
    assert!(cfg_text.contains("terraform plan*"));

    // A brand-new engine loading layered config now auto-allows.
    let loaded = Config::load(&CliOverrides::default(), Some(tmp.path())).unwrap();
    assert!(
        loaded
            .permissions
            .allow
            .iter()
            .any(|r| r == "terraform plan*")
    );
}

#[tokio::test]
async fn outside_root_write_disables_persist_but_prompts() {
    let tmp = tempfile::tempdir().unwrap();
    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("writing"),
        tool_call_delta(
            0,
            Some("w1"),
            Some("write_file"),
            r#"{"path":"../outside.txt","content":"x"}"#
        ),
        finish_json("tool_calls", 4, 4),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("ok"),
        finish_json("stop", 5, 5),
        done()
    ));

    let base = serve(script).await;
    let (handle, mut ev) = spawn(cfg_for(base, tmp.path()));
    handle.submit("write outside");

    let approval = wait_for(
        &mut ev,
        |e| matches!(e, Event::ApprovalRequired { tool, .. } if tool == "write_file"),
    )
    .await;
    let Event::ApprovalRequired {
        id, can_persist, ..
    } = approval
    else {
        unreachable!()
    };
    assert!(!can_persist, "outside-root targets must disable persist");

    handle.approve(id, ApprovalDecision::Once);
    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));
    assert!(tmp.path().parent().unwrap().join("outside.txt").exists());
}

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
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        guarded: false,
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
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        guarded: false,
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

    common::reset_sub_requests();
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
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        guarded: false,
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

#[tokio::test]
async fn reviewer_posts_findings_after_edit_batch() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("calc.txt"), "value = 1\n").unwrap();

    let script = Script::default();
    // R0: read the target first (read-before-edit enforcement).
    script.push(format!(
        "{}{}{}{}",
        text_delta("reading"),
        tool_call_delta(0, Some("rvr"), Some("read_file"), r#"{"path":"calc.txt"}"#),
        finish_json("tool_calls", 5, 5),
        done()
    ));
    // R1: edit a file (gated -> approved once).
    script.push(format!(
        "{}{}{}{}",
        text_delta("editing"),
        tool_call_delta(
            0,
            Some("rv0"),
            Some("edit_file"),
            r#"{"path":"calc.txt","old_string":"value = 1","new_string":"value = 2"}"#
        ),
        finish_json("tool_calls", 10, 10),
        done()
    ));
    // R2 (post-review): model responds to findings.
    script.push(format!(
        "{}{}{}",
        text_delta("addressed."),
        finish_json("stop", 20, 20),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base.clone(), tmp.path());
    cfg.review_enabled = true;
    let (handle, mut ev) = spawn(cfg);
    handle.submit("bump value");

    // Approve the edit.
    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired { id, .. } = approval else {
        unreachable!()
    };
    handle.approve(id, ApprovalDecision::Once);

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));

    // Reviewer ran and its findings entered the parent context.
    let bodies = script.requests_snapshot();
    let reviewer_calls = bodies
        .iter()
        .filter(|b| b.contains("code reviewer"))
        .count();
    assert_eq!(reviewer_calls, 1, "exactly one reviewer side-request");
    assert!(
        bodies
            .iter()
            .any(|b| b.contains("[harness reviewer]") && b.contains("OFF_BY_ONE_RISK")),
        "findings never reached parent context"
    );
}

#[tokio::test]
async fn reviewer_no_findings_stays_silent() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "x\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("e"),
        tool_call_delta(
            0,
            Some("z0"),
            Some("write_file"),
            r#"{"path":"out.txt","content":"ok"}"#
        ),
        finish_json("tool_calls", 5, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("done."),
        finish_json("stop", 8, 8),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base.clone(), tmp.path());
    cfg.review_enabled = true;
    let (handle, mut ev) = spawn(cfg);
    handle.submit("write out NO_FINDINGS_PLEASE");

    let approval = wait_for(&mut ev, |e| matches!(e, Event::ApprovalRequired { .. })).await;
    let Event::ApprovalRequired { id, .. } = approval else {
        unreachable!()
    };
    handle.approve(id, ApprovalDecision::Once);

    let completed = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    assert!(matches!(completed, Event::TurnCompleted { .. }));

    let bodies = script.requests_snapshot();
    assert!(bodies.iter().any(|b| b.contains("code reviewer")));
    assert!(
        !bodies.iter().any(|b| b.contains("[harness reviewer]")),
        "NO_FINDINGS must not inject a message"
    );
}

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

    let loaded = Config::load(&Default::default(), Some(tmp.path())).unwrap();
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

#[tokio::test]
async fn plan_mode_blocks_mutations_without_prompting() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("t.txt"), "original\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}{}",
        text_delta("trying an edit"),
        tool_call_delta(
            0,
            Some("p1"),
            Some("edit_file"),
            r#"{"path":"t.txt","old_string":"original","new_string":"changed"}"#
        ),
        finish_json("tool_calls", 6, 6),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("stayed in plan."),
        finish_json("stop", 7, 7),
        done()
    ));

    let base = serve(script.clone()).await;
    let mut cfg = cfg_for(base, tmp.path());
    cfg.initial_mode = z_engine_core::agent::PermissionMode::Plan;
    let (handle, mut ev) = spawn(cfg);
    handle.submit("try to edit");

    // Drain until completion, asserting no approval ever surfaces.
    let mut saw_approval = false;
    loop {
        match tokio::time::timeout(Duration::from_millis(500), ev.recv()).await {
            Ok(Some(Event::ApprovalRequired { .. })) => saw_approval = true,
            Ok(Some(Event::ToolCallFinished {
                ok: false, summary, ..
            })) if summary.contains("plan mode blocked") => {}
            Ok(Some(Event::TurnCompleted { .. })) => break,
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    assert!(!saw_approval, "plan mode surfaced an approval");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("t.txt")).unwrap(),
        "original\n"
    );
}
