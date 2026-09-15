use std::time::Duration;

use z_engine_core::agent::{ApprovalDecision, Event, spawn};

use crate::mock_loop::{
    Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for,
};

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
async fn always_persist_writes_config_and_never_reprompts() {
    use z_engine_core::config::{Config, project_config_path};
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
    let loaded = Config::load(Some(tmp.path())).unwrap();
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
    let outside = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir_in(outside.path()).unwrap();
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
