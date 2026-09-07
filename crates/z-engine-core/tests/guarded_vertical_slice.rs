//! The vertical slice, end to end and frozen (Task 8).
//!
//! One guarded turn over a real Rust crate: read the file, declare a
//! scoped work order over it, change one function, be refused the second
//! file, and end the turn only because a test the model never saw
//! passes. The fixture lives in `tests/fixtures/guarded-rust-edit/` and
//! is copied per run, so the baseline this locks is a checked-in artifact
//! rather than a string in a test.
//!
//! Kept out of `guarded_completion.rs` and `guarded_replay.rs`
//! deliberately: those files are about who may end a turn and whether a
//! run replays. This one is about the whole slice being reachable at once
//! (AGENTS.md — one concern per integration file).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta};
use z_engine_core::agent::{Event, EventRx, ResumeState, spawn_with_run_recorder};
use z_engine_core::lsp::LspClient;
use z_engine_core::replay::{RecordingProvider, ReplayProvider, RunCassette, RunRecorder};
use z_engine_provider::{ChatProvider, Client};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/guarded-rust-edit"
);

/// The bug the held-out test catches, and the one-function fix for it.
const BUGGY: &str = "text.len()";
const FIXED: &str = "text.split_whitespace().count()";

/// Copy the frozen fixture's sources into `dest`, leaving anything cargo
/// built there in place — resetting for a replay must restore the run's
/// starting state without throwing away a warm `target/`.
fn plant(dest: &Path) {
    copy_tree(Path::new(FIXTURE), dest);
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

/// Whether this host can answer the Rust semantic question the mutation
/// gate asks. The slice is Rust-only and refuses an unproven edit, so
/// without a working provider there is nothing to assert — see
/// `docs/deviations.md`.
///
/// Set `Z_ENGINE_REQUIRE_SEMANTICS=1` where a provider is expected (a
/// release check, a CI image that installs rust-analyzer) and the skip
/// becomes a failure: a baseline that can quietly assert nothing is not a
/// baseline.
async fn semantics_ready(root: &Path) -> bool {
    let ready = match LspClient::probe(root) {
        Some(server) => LspClient::new(root, server).health().await.is_ready(),
        None => false,
    };
    if !ready && std::env::var("Z_ENGINE_REQUIRE_SEMANTICS").is_ok_and(|v| v != "0") {
        panic!(
            "Z_ENGINE_REQUIRE_SEMANTICS is set but no Rust semantic provider \
             answered for {}: install rust-analyzer or unset the variable",
            root.display()
        );
    }
    ready
}

/// The turn the fixture forces: read `src/lib.rs`, scope an order to it
/// citing that evidence, fix the one function it names, be refused the
/// second file, then claim completion.
fn slice_script() -> Script {
    let script = Script::default();
    script.push(round(
        "call_read",
        "read_file",
        r#"{"path":"src/lib.rs"}"#,
        10,
    ));
    script.push(round(
        "call_order",
        "set_work_order",
        r#"{"goal":"count words, not bytes","writable_paths":["src/lib.rs"],"target_symbols":["word_count"],"evidence_ids":["__EVIDENCE_ID__"],"acceptance_commands":[{"command":"cargo test","description":"the held-out test must pass"}]}"#,
        20,
    ));
    script.push(round(
        "call_fix",
        "edit_file",
        &edit_json("src/lib.rs", BUGGY, FIXED),
        30,
    ));
    // The second file: read first, so the refusal that follows can only
    // be about scope — the evidence for it is on the record.
    script.push(round(
        "call_read_stray",
        "read_file",
        r#"{"path":"src/summary.rs"}"#,
        40,
    ));
    // In the same crate, needed by nothing the order declared, and
    // therefore not this turn's to touch.
    script.push(round(
        "call_stray",
        "edit_file",
        &edit_json("src/summary.rs", "{} words", "{} tokens"),
        50,
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("word_count now counts words."),
        finish_json("stop", 60, 5),
        done()
    ));
    script
}

fn round(id: &str, tool: &str, args: &str, prompt_tokens: u64) -> String {
    format!(
        "{}{}{}",
        tool_call_delta(0, Some(id), Some(tool), args),
        finish_json("tool_calls", prompt_tokens, 5),
        done()
    )
}

fn edit_json(path: &str, old: &str, new: &str) -> String {
    serde_json::json!({"path": path, "old_string": old, "new_string": new}).to_string()
}

/// Drive one guarded turn to its end, keeping every event: the refusals
/// this slice is about happen in the middle of the turn, not at its end.
///
/// An empty resume state suppresses the session titler, which
/// `guarded_replay.rs` covers on its own terms.
async fn guarded_turn(
    repo: &Path,
    base: &str,
    provider: Arc<dyn ChatProvider>,
    run: Arc<RunRecorder>,
) -> Vec<Event> {
    let mut cfg = cfg_for(base.to_string(), repo);
    cfg.guarded = true;
    // The gates, not the approval modal, are what this test is about.
    cfg.auto_allow_tools = vec!["read_file".into(), "edit_file".into()];
    let (handle, mut ev) =
        spawn_with_run_recorder(cfg, provider, Some(ResumeState::default()), None, Some(run));
    handle.submit("make word_count count words");
    let events = collect_turn(&mut ev).await;
    drop(handle);
    events
}

/// Every event up to and including the one that ends the turn.
async fn collect_turn(ev: &mut EventRx) -> Vec<Event> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    let mut seen = Vec::new();
    while tokio::time::Instant::now() < deadline {
        let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(500), ev.recv()).await
        else {
            continue;
        };
        let terminal = matches!(
            event,
            Event::TurnCompleted { .. } | Event::TurnBlocked { .. } | Event::Error(_)
        );
        seen.push(event);
        if terminal {
            return seen;
        }
    }
    panic!("the guarded turn never ended: {seen:?}");
}

/// How one tool call ended, in the order the calls were made.
fn outcomes<'a>(events: &'a [Event], tool: &str) -> Vec<(bool, &'a str)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::ToolCallFinished {
                name, ok, summary, ..
            } if name == tool => Some((*ok, summary.as_str())),
            _ => None,
        })
        .collect()
}

fn completed(events: &[Event]) -> bool {
    matches!(events.last(), Some(Event::TurnCompleted { .. }))
}

/// The manifest the completion gate wrote for the run under `root`.
fn manifest_of(root: &Path) -> serde_json::Value {
    let runs = root.join(".z-engine").join("runs");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&runs)
        .unwrap_or_else(|e| panic!("no guarded run directory under {}: {e}", runs.display()))
        .filter_map(|e| e.ok().map(|e| e.path().join("verification.json")))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    let path = found
        .pop()
        .expect("a completed guarded turn must leave a verification manifest");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Record one slice run onto a cassette and hand back everything the
/// assertions need.
async fn record_slice(repo: &Path, vault: &Path) -> (Vec<Event>, Script, String, PathBuf) {
    let script = slice_script();
    let base = serve(script.clone()).await;
    // Cassettes live outside the workspace: a guarded run audits every
    // change inside it, and its own recording is not part of the work.
    let tape = vault.join("recorded.jsonl");
    let recorder = RunRecorder::recording(&tape).unwrap();
    let live = Arc::new(Client::new(&base, Some("test-key-not-real".into())).unwrap());
    let taping = Arc::new(RecordingProvider::new(live, Arc::clone(&recorder)));
    let events = guarded_turn(repo, &base, taping, recorder).await;
    (events, script, base, tape)
}

/// The slice itself: evidence, a scoped order, one function changed, the
/// second file refused, and a test the model never saw deciding whether
/// the turn may end.
#[tokio::test]
async fn the_guarded_slice_gates_a_rust_edit_on_evidence_scope_and_a_held_out_test() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    plant(repo.path());
    if !semantics_ready(repo.path()).await {
        eprintln!("no Rust semantic provider on this host; skipping the guarded slice");
        return;
    }
    let (events, ..) = record_slice(repo.path(), vault.path()).await;

    assert!(
        completed(&events),
        "the verified change must end the turn: {events:?}"
    );
    assert_eq!(
        outcomes(&events, "set_work_order")
            .first()
            .map(|(ok, _)| *ok),
        Some(true),
        "the order citing fresh evidence must be admitted: {events:?}"
    );

    let edits = outcomes(&events, "edit_file");
    assert_eq!(edits.len(), 2, "both edits must be answered: {events:?}");
    assert!(edits[0].0, "the in-scope fix must land: {}", edits[0].1);
    assert!(
        !edits[1].0,
        "the second file is outside the order's scope: {}",
        edits[1].1
    );
    assert!(
        edits[1].1.contains("src/summary.rs") && edits[1].1.contains("scope"),
        "the refusal must name the file and the reason: {}",
        edits[1].1
    );

    assert!(
        read(repo.path(), "src/lib.rs").contains(FIXED),
        "the authorized change must be on disk"
    );
    assert_eq!(
        read(repo.path(), "src/summary.rs"),
        std::fs::read_to_string(Path::new(FIXTURE).join("src/summary.rs")).unwrap(),
        "a refused edit must leave nothing behind"
    );
    assert_eq!(
        read(repo.path(), "tests/held_out.rs"),
        std::fs::read_to_string(Path::new(FIXTURE).join("tests/held_out.rs")).unwrap(),
        "the verification must stay held out — the run may not edit the test that judges it"
    );

    // The turn ended because cargo said so, not because the model did.
    let manifest = manifest_of(repo.path());
    assert_eq!(manifest["complete"], serde_json::json!(true), "{manifest}");
    let held_out = manifest["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["command"].as_str().is_some_and(|s| s == "cargo test"))
        .expect("the declared acceptance command must appear in the manifest");
    assert_eq!(
        held_out["status"]["status"],
        serde_json::json!("passed"),
        "the held-out test must actually have run and passed: {held_out}"
    );
}

/// The baseline is locked only if it can be re-run: the recorded slice
/// must replay to the same verdict, with the same manifest, from the
/// cassette alone.
#[tokio::test]
async fn the_recorded_slice_replays_to_the_same_verdict_without_a_network() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    plant(repo.path());
    if !semantics_ready(repo.path()).await {
        eprintln!("no Rust semantic provider on this host; skipping the guarded slice replay");
        return;
    }
    let (events, script, base, tape) = record_slice(repo.path(), vault.path()).await;
    assert!(
        completed(&events),
        "the recorded run must verify: {events:?}"
    );
    let served = script.request_count();

    plant(repo.path());
    let recorded = RunCassette::load(&tape).unwrap();
    let replaying = Arc::new(ReplayProvider::new(&recorded).unwrap());
    let replay_tape = vault.path().join("replayed.jsonl");
    let recorder = RunRecorder::replaying(&replay_tape, &recorded).unwrap();
    let taping = Arc::new(RecordingProvider::new(
        Arc::clone(&replaying) as Arc<dyn ChatProvider>,
        Arc::clone(&recorder),
    ));
    let replayed_events = guarded_turn(repo.path(), &base, taping, recorder).await;

    assert!(
        completed(&replayed_events),
        "the replayed run must reach the same completion: {replayed_events:?}"
    );
    assert!(
        replaying.mismatch().is_none(),
        "replay diverged: {:?}",
        replaying.mismatch()
    );
    assert_eq!(
        script.request_count(),
        served,
        "replay must not have touched the network"
    );

    let replayed = RunCassette::load(&replay_tape).unwrap();
    assert_eq!(recorded.request_hashes(), replayed.request_hashes());
    assert_eq!(recorded.gate_decisions(), replayed.gate_decisions());
    assert_eq!(recorded.evidence_ids(), replayed.evidence_ids());
    assert_eq!(
        recorded.manifest_hash(),
        replayed.manifest_hash(),
        "the same verification verdict must have been reached"
    );
    assert!(recorded.manifest_hash().is_some(), "both runs verified");
    assert_eq!(
        recorded.diff_hash(),
        replayed.diff_hash(),
        "the workspace must have ended in the same state"
    );
    let (a, b) = (recorded.metrics().unwrap(), replayed.metrics().unwrap());
    assert_eq!(a.deterministic(), b.deterministic(), "{a:?} vs {b:?}");
    assert_eq!(a.outcome, "completed");
    // read, order, fix, read, refused edit.
    assert_eq!(a.tool_calls, 5);
}
