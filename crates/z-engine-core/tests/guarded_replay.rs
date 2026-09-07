//! Record-and-replay tests (Task 7): a guarded run must be reproducible
//! from its cassette alone — same requests, same tool outcomes, same gate
//! decisions, same diff, same metrics — with no network in sight.
//!
//! Kept out of `guarded_completion.rs` deliberately: that file is about
//! who may end a turn; this one is about whether a finished run can be
//! played back byte-for-byte (AGENTS.md — one concern per integration
//! file).

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for};
use z_engine_core::agent::{Event, ResumeState, spawn_with_run_recorder};
use z_engine_core::replay::{RecordingProvider, ReplayProvider, RunCassette, RunRecorder};
use z_engine_provider::{ChatProvider, Client};

const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";
const MANIFEST: &str = "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";

/// A minimal, dependency-free cargo project so the completion gate runs a
/// real (fast, offline) `cargo check` in both the recorded and the
/// replayed run — replay must not weaken a gate.
fn fixture(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), LIB).unwrap();
    std::fs::write(root.join("Cargo.toml"), MANIFEST).unwrap();
}

/// One guarded turn: read the manifest, declare the order over it citing
/// the evidence just minted, write the change, then claim completion.
fn guarded_script(content: &str) -> Script {
    let script = Script::default();
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
            r#"{"goal":"describe the fixture","writable_paths":["Cargo.toml"],"target_symbols":[],"evidence_ids":["__EVIDENCE_ID__"],"acceptance_commands":[{"command":"cargo check","description":"acceptance"}]}"#
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
        text_delta("The manifest now carries a description."),
        finish_json("stop", 40, 5),
        done()
    ));
    script
}

/// Drive one guarded turn through `provider`, recording into `run`.
///
/// A resume state (empty) is passed deliberately: it suppresses the
/// session-title side request, whose interleaving with the turn's own
/// request is scheduler-dependent and therefore not replayable in
/// sequence.
async fn guarded_turn(
    repo: &Path,
    base: &str,
    provider: Arc<dyn ChatProvider>,
    run: Arc<RunRecorder>,
    prompt: &str,
) -> Event {
    let mut cfg = cfg_for(base.to_string(), repo);
    cfg.guarded = true;
    // The completion gate, not the approval modal, is what this is about.
    cfg.auto_allow_tools = vec!["write_file".into(), "read_file".into()];
    let (handle, mut ev) =
        spawn_with_run_recorder(cfg, provider, Some(ResumeState::default()), None, Some(run));
    handle.submit(prompt);
    let outcome = wait_for(&mut ev, |e| {
        matches!(
            e,
            Event::TurnCompleted { .. } | Event::TurnBlocked { .. } | Event::Error(_)
        )
    })
    .await;
    // Keep the handle alive until the turn settles, then let it go.
    drop(handle);
    outcome
}

/// The round trip: record a guarded edit, reset the fixture, replay it
/// with no HTTP at all, and prove the second run is the first one.
#[tokio::test]
async fn a_guarded_run_replays_from_its_cassette_without_touching_the_network() {
    let repo = tempfile::tempdir().unwrap();
    // Cassettes live outside the workspace: a guarded run audits every
    // change inside it, and its own recording is not part of the work.
    let vault = tempfile::tempdir().unwrap();
    fixture(repo.path());
    let changed = format!("{MANIFEST}description = \"fixture crate\"\n");

    let script = guarded_script(&changed);
    let base = serve(script.clone()).await;

    // ---- record against the live (mock HTTP) provider -----------------
    let recorded_path = vault.path().join("recorded.jsonl");
    let recorder = RunRecorder::recording(&recorded_path).unwrap();
    let live = Arc::new(Client::new(&base, Some("test-key-not-real".into())).unwrap());
    let taping = Arc::new(RecordingProvider::new(live, Arc::clone(&recorder)));
    let outcome = guarded_turn(
        repo.path(),
        &base,
        taping,
        Arc::clone(&recorder),
        "add a description to the fixture manifest",
    )
    .await;
    assert!(
        matches!(outcome, Event::TurnCompleted { .. }),
        "the recorded run must verify and complete: {outcome:?}"
    );
    assert!(
        std::fs::read_to_string(repo.path().join("Cargo.toml"))
            .unwrap()
            .contains("description"),
        "the recorded run must actually have changed the workspace"
    );
    let served_during_record = script.request_count();
    assert!(served_during_record >= 4, "the run must have used the wire");

    // ---- reset the fixture -------------------------------------------
    fixture(repo.path());

    // ---- replay: the cassette is the only transport -------------------
    let recorded = RunCassette::load(&recorded_path).unwrap();
    let replayed_path = vault.path().join("replayed.jsonl");
    let replaying = Arc::new(ReplayProvider::new(&recorded));
    let replay_recorder = RunRecorder::replaying(&replayed_path, &recorded).unwrap();
    let taping = Arc::new(RecordingProvider::new(
        Arc::clone(&replaying) as Arc<dyn ChatProvider>,
        Arc::clone(&replay_recorder),
    ));
    let outcome = guarded_turn(
        repo.path(),
        &base,
        taping,
        Arc::clone(&replay_recorder),
        "add a description to the fixture manifest",
    )
    .await;
    assert!(
        matches!(outcome, Event::TurnCompleted { .. }),
        "the replayed run must reach the same completion: {outcome:?}"
    );
    assert_eq!(
        script.request_count(),
        served_during_record,
        "replay must not have touched the network"
    );
    assert!(
        replaying.mismatch().is_none(),
        "replay diverged: {:?}",
        replaying.mismatch()
    );

    // ---- the two runs must be the same run ----------------------------
    let replayed = RunCassette::load(&replayed_path).unwrap();
    assert_eq!(
        recorded.request_hashes(),
        replayed.request_hashes(),
        "every request must be byte-identical, in order"
    );
    assert_eq!(recorded.request_hashes().len(), 4);
    assert_eq!(recorded.prompt_hashes(), replayed.prompt_hashes());
    assert_eq!(
        recorded.tool_outcomes(),
        replayed.tool_outcomes(),
        "the same tools must have produced the same results"
    );
    assert_eq!(recorded.tool_outcomes().len(), 3);
    assert_eq!(
        recorded.gate_decisions(),
        replayed.gate_decisions(),
        "the mutation gate must have decided the same way"
    );
    assert!(
        recorded.gate_decisions().iter().any(|d| d.allowed),
        "the recorded run must have passed the mutation gate at least once"
    );
    assert_eq!(recorded.evidence_ids(), replayed.evidence_ids());
    assert_eq!(
        recorded.diff_hash(),
        replayed.diff_hash(),
        "the workspace must have ended in the same state"
    );
    assert!(recorded.diff_hash().is_some(), "a change was made");
    assert_eq!(
        recorded.manifest_hash(),
        replayed.manifest_hash(),
        "the same verification verdict must have been reached"
    );
    assert!(
        recorded.manifest_hash().is_some(),
        "a completed guarded run must have verified, in both runs"
    );

    let (a, b) = (recorded.metrics().unwrap(), replayed.metrics().unwrap());
    assert_eq!(a.deterministic(), b.deterministic(), "{a:?} vs {b:?}");
    assert_eq!(a.model_id, "test-model");
    assert_eq!(a.turns, 1);
    assert_eq!(a.tool_calls, 3);
    assert_eq!(a.input_tokens, 100);
    assert_eq!(a.output_tokens, 20);
    assert_eq!(a.outcome, "completed");
}

/// A cassette is not a suggestion: the first request that does not match
/// what was recorded fails with its sequence number, and nothing falls
/// back to the network.
#[tokio::test]
async fn a_diverging_replay_fails_at_the_first_mismatch_with_its_sequence() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("notes.md"), "# notes\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        text_delta("nothing to do."),
        finish_json("stop", 7, 3),
        done()
    ));
    let base = serve(script.clone()).await;

    let recorded_path = vault.path().join("recorded.jsonl");
    let recorder = RunRecorder::recording(&recorded_path).unwrap();
    let live = Arc::new(Client::new(&base, Some("test-key-not-real".into())).unwrap());
    let taping = Arc::new(RecordingProvider::new(live, Arc::clone(&recorder)));
    let (handle, mut ev) = spawn_with_run_recorder(
        cfg_for(base.clone(), repo.path()),
        taping,
        Some(ResumeState::default()),
        None,
        Some(Arc::clone(&recorder)),
    );
    handle.submit("say hello");
    let _ = wait_for(&mut ev, |e| matches!(e, Event::TurnCompleted { .. })).await;
    let served = script.request_count();
    drop(handle);

    // Replay the same cassette against a *different* prompt.
    let recorded = RunCassette::load(&recorded_path).unwrap();
    assert_eq!(
        recorded.request_hashes().len(),
        1,
        "an empty tape would make the mismatch below meaningless"
    );
    let replaying = Arc::new(ReplayProvider::new(&recorded));
    let (handle, mut ev) = spawn_with_run_recorder(
        cfg_for(base, repo.path()),
        Arc::clone(&replaying) as Arc<dyn ChatProvider>,
        Some(ResumeState::default()),
        None,
        None,
    );
    handle.submit("say something else entirely");
    let reported = wait_for(&mut ev, |e| {
        matches!(e, Event::Error(_) | Event::TurnCompleted { .. })
    })
    .await;
    let Event::Error(message) = reported else {
        panic!("a diverging replay must never look like a completed turn: {reported:?}");
    };
    assert!(
        message.contains("replay") && message.contains('0'),
        "the failure must name the sequence it diverged at: {message}"
    );

    let mismatch = replaying.mismatch().expect("the divergence must be typed");
    assert_eq!(mismatch.sequence, 0);
    assert_ne!(mismatch.expected_hash, mismatch.actual_hash);
    assert_eq!(
        script.request_count(),
        served,
        "a mismatch must never fall back to the network"
    );
}
