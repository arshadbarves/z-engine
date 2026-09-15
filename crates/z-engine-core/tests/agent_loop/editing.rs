use z_engine_core::agent::{ApprovalDecision, Event, spawn};

use crate::mock_loop::{
    Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for,
};

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
