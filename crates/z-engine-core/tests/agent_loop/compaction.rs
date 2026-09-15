use z_engine_core::agent::{Event, LoopConfig, spawn_with_recorder};
use z_engine_core::session::SessionWriter;

use crate::mock_loop::{Script, done, finish_json, serve, text_delta, tool_call_delta, wait_for};

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
        max_task_continuations: 0,
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 4,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        shell_path: None,
    };
    let recorder = SessionWriter::append_to(&tmp.path().join("transcript.jsonl")).unwrap();
    let (handle, mut ev) = spawn_with_recorder(cfg, None, Some(recorder));
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
    // The fact must be in an attributed summary, not just an earlier tool output.
    assert!(
        bodies.iter().any(|body| {
            let request: serde_json::Value = serde_json::from_str(body).unwrap();
            request["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| {
                    message["content"]
                        .as_str()
                        .and_then(|content| {
                            serde_json::from_str::<z_engine_context::ContextPacket>(content).ok()
                        })
                        .is_some_and(|packet| {
                            packet.model_notes.iter().any(|note| {
                                note.text.contains("THE_SECRET_ZEBRA_GRAZES_AT_NOON")
                                    && note.trust == z_engine_context::NoteTrust::Unverified
                            })
                        })
                })
        }),
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
