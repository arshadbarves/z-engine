//! One tape, several callers, and the holes a tape must not have
//! (Task 7 fix round 1).
//!
//! A guarded run is not one conversation. The turn loop talks to the
//! provider while a session titler, a reviewer, a compactor and any
//! sub-agents talk to the same one. Recorded on a single sequence their
//! requests interleave differently every run and the tape is unplayable
//! by the second run; recorded per lane each caller is ordered against
//! itself and against nothing else.
//!
//! The other half of that promise is that a lane's sequence is whole.
//! A tape missing a request in the middle would serve the *next* one in
//! its place and report the divergence against a run that did nothing
//! wrong, so a hole is refused before a replay can start.

mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for};
use z_engine_core::agent::{Event, PermissionMode, ResumeState, spawn_with_run_recorder};
use z_engine_core::replay::{
    RecordingProvider, ReplayError, ReplayProvider, RunCassette, RunRecorder,
};
use z_engine_provider::{ChatProvider, Client, RequestLane};

fn live(base: &str) -> Arc<dyn ChatProvider> {
    Arc::new(Client::new(base, Some("test-key-not-real".into())).unwrap())
}

/// One plain turn: the model answers and stops.
fn answering_script() -> Script {
    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        text_delta("nothing needed here."),
        finish_json("stop", 11, 3),
        done()
    ));
    script
}

/// Drive one turn and return how it ended.
async fn turn(
    repo: &Path,
    base: &str,
    provider: Arc<dyn ChatProvider>,
    run: Option<Arc<RunRecorder>>,
    mode: PermissionMode,
    resume: Option<ResumeState>,
    prompt: &str,
) -> Event {
    let mut cfg = cfg_for(base.to_string(), repo);
    cfg.initial_mode = mode;
    let (handle, mut ev) = spawn_with_run_recorder(cfg, provider, resume, None, run);
    handle.submit(prompt);
    let outcome = wait_for(&mut ev, |e| {
        matches!(
            e,
            Event::TurnCompleted { .. } | Event::TurnBlocked { .. } | Event::Error(_)
        )
    })
    .await;
    drop(handle);
    outcome
}

// ---------------------------------------------------------------------
// Finding 3 — concurrent callers each get their own lane
/// A run that was not resumed titles itself, and the titler talks to the
/// same provider *beside* the turn rather than inside it. Recorded on one
/// sequence the two would interleave differently every run; recorded per
/// lane, the run replays whichever order they arrive in this time.
#[tokio::test]
async fn a_fresh_run_replays_even_though_the_titler_runs_beside_the_turn() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("notes.md"), "# notes\n").unwrap();
    let script = answering_script();
    let base = serve(script.clone()).await;

    let recorded_path = vault.path().join("recorded.jsonl");
    let recorder = RunRecorder::recording(&recorded_path).unwrap();
    let taping = Arc::new(RecordingProvider::new(live(&base), Arc::clone(&recorder)));
    // No resume state: this run names itself.
    let outcome = turn(
        repo.path(),
        &base,
        taping,
        Some(Arc::clone(&recorder)),
        PermissionMode::Normal,
        None,
        "say hello",
    )
    .await;
    assert!(
        matches!(outcome, Event::TurnCompleted { .. }),
        "the recorded run must complete: {outcome:?}"
    );

    let recorded = RunCassette::load(&recorded_path).unwrap();
    let title_lane = RequestLane::named("title");
    assert_eq!(
        recorded.exchanges_on(&RequestLane::MAIN).len(),
        1,
        "the turn made one request"
    );
    assert_eq!(
        recorded.exchanges_on(&title_lane).len(),
        1,
        "the titler's request must be taped too, on its own lane"
    );
    assert_eq!(recorded.lanes().len(), 2, "{:?}", recorded.lanes());

    // ---- replay: the cassette is the only transport -------------------
    let replaying = Arc::new(ReplayProvider::new(&recorded).unwrap());
    let served_before = script.request_count();
    let outcome = turn(
        repo.path(),
        &base,
        Arc::clone(&replaying) as Arc<dyn ChatProvider>,
        None,
        PermissionMode::Normal,
        None,
        "say hello",
    )
    .await;
    assert!(
        matches!(outcome, Event::TurnCompleted { .. }),
        "a fresh recording must replay: {outcome:?}"
    );
    assert!(
        replaying.mismatch().is_none(),
        "replay diverged: {:?}",
        replaying.mismatch()
    );
    assert_eq!(
        script.request_count(),
        served_before,
        "replay must not have touched the network"
    );
    // The titler is spawned, not awaited, so *when* it is served is the
    // scheduler's business; that both lanes were served is the run's.
    assert_eq!(replaying.served(), 2);
}

// ---------------------------------------------------------------------
// ---------------------------------------------------------------------
// Finding 4 — a tape with a hole is refused before it is served
// ---------------------------------------------------------------------

/// Drop the exchange at `sequence` from a cassette file, leaving a run
/// that looks whole line-by-line and is not.
fn drop_exchange(path: &Path, sequence: u64) {
    let kept: Vec<String> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            !(value["entry"] == "exchange" && value["sequence"] == sequence)
        })
        .map(str::to_string)
        .collect();
    std::fs::write(path, format!("{}\n", kept.join("\n"))).unwrap();
}

/// A cassette missing a request in the middle would otherwise serve the
/// *next* one in its place, and report the divergence against a run that
/// did nothing wrong.
#[tokio::test]
async fn a_cassette_with_a_missing_request_is_refused_before_it_can_replay() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("notes.md"), "# notes\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        tool_call_delta(0, Some("call_glob"), Some("glob"), r#"{"pattern":"*.md"}"#),
        finish_json("tool_calls", 10, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("found the notes."),
        finish_json("stop", 20, 5),
        done()
    ));
    let base = serve(script.clone()).await;

    let path = vault.path().join("holed.jsonl");
    let recorder = RunRecorder::recording(&path).unwrap();
    let taping = Arc::new(RecordingProvider::new(live(&base), Arc::clone(&recorder)));
    let outcome = turn(
        repo.path(),
        &base,
        taping,
        Some(Arc::clone(&recorder)),
        PermissionMode::Normal,
        Some(ResumeState::default()),
        "list the markdown files",
    )
    .await;
    assert!(
        matches!(outcome, Event::TurnCompleted { .. }),
        "the recorded run must complete: {outcome:?}"
    );
    assert_eq!(
        RunCassette::load(&path).unwrap().exchanges().len(),
        2,
        "a one-request tape could not have a hole in the middle"
    );

    drop_exchange(&path, 0);
    let holed = RunCassette::load(&path).expect("every remaining line is still well-formed");
    assert_eq!(holed.exchanges().len(), 1);

    let err = ReplayProvider::new(&holed).unwrap_err();
    let ReplayError::SequenceGap {
        lane,
        expected,
        found,
        ..
    } = &err
    else {
        panic!("a tape with a hole must be refused at construction: {err}");
    };
    assert_eq!((lane.as_str(), *expected, *found), ("main", 0, 1));
}

/// The same refusal, reported at the *first* hole rather than the last —
/// a later gap is downstream of the first and would bury the cause.
#[tokio::test]
async fn a_gap_is_reported_where_it_starts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run.jsonl");
    let recorder = RunRecorder::recording(&path).unwrap();
    let abort = Arc::new(AtomicBool::new(false));
    let inner: Arc<dyn ChatProvider> = Arc::new(Silent);
    let taping = RecordingProvider::new(inner, Arc::clone(&recorder));
    for i in 0..4 {
        let request = z_engine_provider::ChatRequest::new(
            "m",
            vec![z_engine_provider::ChatMessage::user(format!("ask {i}"))],
        );
        let mut rx = taping.stream_chat(&request, Arc::clone(&abort));
        while rx.recv().await.is_some() {}
    }
    recorder.settle().await;
    assert_eq!(RunCassette::load(&path).unwrap().exchanges().len(), 4);

    drop_exchange(&path, 1);
    drop_exchange(&path, 3);
    let err = ReplayProvider::new(&RunCassette::load(&path).unwrap()).unwrap_err();
    assert!(
        matches!(
            &err,
            ReplayError::SequenceGap {
                expected: 1,
                found: 2,
                ..
            }
        ),
        "the first hole is the one that matters: {err}"
    );
}

/// A provider that answers with nothing at all — enough to tape an
/// exchange without a mock server in the way.
#[derive(Debug)]
struct Silent;

impl ChatProvider for Silent {
    fn stream_chat_on(
        &self,
        _lane: &RequestLane,
        _request: &z_engine_provider::ChatRequest,
        _abort: Arc<AtomicBool>,
    ) -> z_engine_provider::EventStream {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.try_send(Ok(z_engine_provider::StreamEvent::Done));
        rx
    }

    fn set_api_key(&self, _key: Option<String>) {}
}
