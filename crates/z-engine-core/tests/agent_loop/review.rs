use z_engine_core::agent::{ApprovalDecision, Event, spawn};

use crate::mock_loop::{
    Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for,
};

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
            .any(|b| b.contains("review_findings") && b.contains("OFF_BY_ONE_RISK")),
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
        !bodies.iter().any(|b| b.contains("review_findings")),
        "NO_FINDINGS must not inject a message"
    );
}
