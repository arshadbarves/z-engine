//! Guarded-mode completion tests (Task 6): in a guarded run that changed
//! the workspace, the model's final answer is a *claim*, not a verdict.
//! Only a complete, all-passing verification manifest may end the turn.
//!
//! Kept out of `agent_loop_mocked.rs` deliberately: that file is already
//! far past the repository's file budget, and completion gating is its own
//! concern (AGENTS.md — one concern per integration file).

mod common;

use std::path::Path;

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for};
use z_engine_core::agent::{Event, spawn};

const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";
const MANIFEST: &str = "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";

/// A minimal, dependency-free cargo project so `cargo check` is a real
/// (fast, offline) verdict rather than a mock.
fn fixture(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), LIB).unwrap();
    std::fs::write(root.join("Cargo.toml"), MANIFEST).unwrap();
}

/// Round 1 reads `Cargo.toml`, round 2 declares the order over it citing
/// the evidence the harness just minted, round 3 writes `content`, round 4
/// answers in prose — the false completion this gate exists to catch.
fn script_for(content: &str, acceptance: &str) -> Script {
    let script = Script::default();
    push_turn(&script, content, acceptance);
    script
}

/// The four rounds one guarded turn takes. Split out so a multi-turn
/// script is the same turn twice rather than a second transcript that
/// could drift from the first.
fn push_turn(script: &Script, content: &str, acceptance: &str) {
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("call_read"),
            Some("read_file"),
            r#"{"path":"Cargo.toml"}"#
        ),
        finish_json("tool_calls", 10, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("call_order"),
            Some("set_work_order"),
            &format!(
                r#"{{"goal":"describe the fixture","writable_paths":["Cargo.toml"],"target_symbols":[],"evidence_ids":["__EVIDENCE_ID__"],"acceptance_commands":[{{"command":"{acceptance}","description":"acceptance"}}]}}"#
            )
        ),
        finish_json("tool_calls", 20, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("call_write"),
            Some("write_file"),
            &format!(
                "{{\"path\":\"Cargo.toml\",\"content\":{}}}",
                serde_json::to_string(content).unwrap()
            )
        ),
        finish_json("tool_calls", 30, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("All done — the change is complete and everything passes."),
        finish_json("stop", 40, 5),
        done()
    ));
}

async fn run_guarded(
    root: &Path,
    script: Script,
) -> (
    z_engine_core::agent::AgentHandle,
    z_engine_core::agent::EventRx,
) {
    let base = serve(script).await;
    let mut cfg = cfg_for(base, root);
    cfg.guarded = true;
    // The gate, not the approval modal, is what this test is about.
    cfg.auto_allow_tools = vec!["write_file".into(), "read_file".into()];
    let (handle, ev) = spawn(cfg);
    handle.submit("add a description to the fixture manifest");
    (handle, ev)
}

/// The false-completion case: the run broke the build and then said it was
/// done. The turn must end blocked at the completion gate, and must never
/// emit `TurnCompleted`.
#[tokio::test]
async fn guarded_completion_is_blocked_when_the_workspace_no_longer_builds() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let broken = "[package\nname = \"fixture\"\n";

    let (_handle, mut ev) = run_guarded(tmp.path(), script_for(broken, "cargo check")).await;

    let blocked = wait_for(&mut ev, |e| {
        matches!(e, Event::TurnBlocked { .. } | Event::TurnCompleted { .. })
    })
    .await;
    let Event::TurnBlocked {
        gate,
        reason,
        manifest_path,
    } = blocked
    else {
        panic!("a broken build must not complete the turn: {blocked:?}");
    };
    assert_eq!(gate, "completion");
    assert!(
        reason.contains("cargo check"),
        "the refusal must name the failing check: {reason}"
    );

    // The refusal has to be auditable, and the manifest has to show a real
    // cargo invocation — otherwise this test would pass on a stub.
    let path = manifest_path.expect("a refusal must point at its evidence");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(manifest["complete"], serde_json::json!(false));
    let check = manifest["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["command"]
                .as_str()
                .is_some_and(|s| s.starts_with("cargo check"))
        })
        .expect("cargo check must be recorded");
    assert_eq!(
        check["status"]["status"],
        serde_json::json!("failed"),
        "cargo must actually have run and failed: {check}"
    );
    assert!(
        check["outputTail"]
            .as_str()
            .is_some_and(|t| t.contains("Cargo.toml")),
        "the tail must carry cargo's own words: {check}"
    );
}

/// The honest case: the change still builds and the declared acceptance
/// command passes, so the manifest is complete and the turn may finish.
#[tokio::test]
async fn guarded_completion_passes_when_verification_is_complete() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let good = format!("{MANIFEST}description = \"fixture crate\"\n");

    let (_handle, mut ev) = run_guarded(tmp.path(), script_for(&good, "cargo check")).await;

    let finished = wait_for(&mut ev, |e| {
        matches!(e, Event::TurnBlocked { .. } | Event::TurnCompleted { .. })
    })
    .await;
    assert!(
        matches!(finished, Event::TurnCompleted { .. }),
        "a verified change must complete: {finished:?}"
    );
}

/// An acceptance command the harness will not execute proves nothing, so
/// the order carrying it is refused where the model can still fix it: at
/// admission. Nothing it would have authorized may reach disk.
#[tokio::test]
async fn guarded_admission_refuses_an_unrunnable_acceptance_command() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let good = format!("{MANIFEST}description = \"fixture crate\"\n");

    let (_handle, mut ev) =
        run_guarded(tmp.path(), script_for(&good, "echo everything is fine")).await;

    let refusal = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "set_work_order"),
    )
    .await;
    let Event::ToolCallFinished { ok, summary, .. } = refusal else {
        unreachable!()
    };
    assert!(!ok, "an unrunnable acceptance command cannot be admitted");
    assert!(summary.contains("echo"), "{summary}");

    // And the write the order would have authorized never lands: without
    // an admitted order the mutation gate has nothing to authorize it.
    wait_for(&mut ev, |e| {
        matches!(e, Event::TurnBlocked { .. } | Event::TurnCompleted { .. })
    })
    .await;
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Cargo.toml")).unwrap(),
        MANIFEST,
        "a refused order must not leave its change behind"
    );
}

/// A cargo subcommand that is not a check — `cargo run`, `cargo install`,
/// anything that executes or fetches code — is not evidence either, and is
/// refused by the same rule rather than by a program allowlist that would
/// wave it through.
#[tokio::test]
async fn guarded_admission_refuses_a_side_effecting_cargo_subcommand() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let good = format!("{MANIFEST}description = \"fixture crate\"\n");

    let (_handle, mut ev) =
        run_guarded(tmp.path(), script_for(&good, "cargo run --bin fixture")).await;

    let refusal = wait_for(
        &mut ev,
        |e| matches!(e, Event::ToolCallFinished { name, .. } if name == "set_work_order"),
    )
    .await;
    let Event::ToolCallFinished { ok, summary, .. } = refusal else {
        unreachable!()
    };
    assert!(!ok, "`cargo run` executes the code it is meant to judge");
    assert!(summary.contains("run"), "{summary}");
}

/// A guarded run is more than one turn, and the first turn's *own*
/// verification writes to the workspace: `cargo check` resolves the
/// dependency graph and leaves a `Cargo.lock` behind.
///
/// If the run kept judging every turn against the tree it started in,
/// that lockfile would open turn 2 as a change no governed tool
/// recorded, and no guarded session could ever complete twice. This is
/// the end-to-end proof that it can.
#[tokio::test]
async fn a_guarded_session_can_complete_more_than_one_turn() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let first = format!("{MANIFEST}description = \"fixture crate\"\n");
    let second = format!("{MANIFEST}description = \"fixture crate\"\nkeywords = [\"fixture\"]\n");

    let script = Script::default();
    push_turn(&script, &first, "cargo check");
    push_turn(&script, &second, "cargo check");

    let (handle, mut ev) = run_guarded(tmp.path(), script).await;

    let turn_one = wait_for(&mut ev, |e| {
        matches!(e, Event::TurnBlocked { .. } | Event::TurnCompleted { .. })
    })
    .await;
    assert!(
        matches!(turn_one, Event::TurnCompleted { .. }),
        "the first turn must verify: {turn_one:?}"
    );
    // Without this the test could pass vacuously: it is the verifier's
    // own write that turn 2 used to be charged for.
    assert!(
        tmp.path().join("Cargo.lock").exists(),
        "verification must actually have written a lockfile"
    );

    handle.submit("now add a keyword too");
    let turn_two = wait_for(&mut ev, |e| {
        matches!(e, Event::TurnBlocked { .. } | Event::TurnCompleted { .. })
    })
    .await;
    assert!(
        matches!(turn_two, Event::TurnCompleted { .. }),
        "a second governed turn must not answer for the first turn's checks: {turn_two:?}"
    );
    assert!(
        std::fs::read_to_string(tmp.path().join("Cargo.toml"))
            .unwrap()
            .contains("keywords"),
        "the second turn's change must have landed"
    );
}
