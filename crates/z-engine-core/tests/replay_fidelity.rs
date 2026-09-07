//! What a cassette must contain, and what it must refuse (Task 7 fix
//! round 1).
//!
//! `guarded_replay.rs` asks whether a finished run plays back; this file
//! asks whether the tape is the run at all: a lost entry condemns the
//! recording rather than being logged past, and every decision the
//! model's trajectory turned on is on the tape and in the count —
//! including the calls that were refused before they could run.
//!
//! `replay_lanes.rs` carries the other half: concurrent callers, and
//! tapes with holes in them.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::{Script, cfg_for, done, finish_json, serve, text_delta, tool_call_delta, wait_for};
use z_engine_core::agent::{Event, PermissionMode, ResumeState, spawn_with_run_recorder};
use z_engine_core::replay::{
    CassetteEntry, EntrySink, RecordingProvider, ReplayError, RunCassette, RunRecorder,
    ToolDisposition,
};
use z_engine_provider::{ChatProvider, Client};

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
// Finding 1 — recording fails closed
// ---------------------------------------------------------------------

/// A sink that writes `budget` entries and then reports the disk full,
/// like a cassette that runs out of space mid-run.
#[derive(Debug)]
struct Failing {
    path: PathBuf,
    budget: Mutex<usize>,
}

impl Failing {
    fn new(path: &Path, budget: usize) -> Self {
        std::fs::write(path, "").unwrap();
        Self {
            path: path.to_path_buf(),
            budget: Mutex::new(budget),
        }
    }
}

impl EntrySink for Failing {
    fn append(&self, entry: &CassetteEntry) -> Result<(), ReplayError> {
        let mut left = self.budget.lock().unwrap();
        if *left == 0 {
            return Err(ReplayError::Io {
                path: self.path.clone(),
                source: std::io::Error::other("no space left on device"),
            });
        }
        *left -= 1;
        let mut line = serde_json::to_string(entry).unwrap();
        line.push('\n');
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)
            .unwrap()
            .write_all(line.as_bytes())
            .unwrap();
        Ok(())
    }
}

/// A run that cannot record itself must not report success, and must not
/// leave a cassette that passes for whole.
#[tokio::test]
async fn a_run_that_loses_an_entry_fails_instead_of_claiming_completion() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("notes.md"), "# notes\n").unwrap();
    let script = answering_script();
    let base = serve(script.clone()).await;

    let path = vault.path().join("doomed.jsonl");
    let recorder = RunRecorder::onto(&path, Box::new(Failing::new(&path, 1)));
    let taping = Arc::new(RecordingProvider::new(live(&base), Arc::clone(&recorder)));
    let outcome = turn(
        repo.path(),
        &base,
        taping,
        Some(Arc::clone(&recorder)),
        PermissionMode::Normal,
        Some(ResumeState::default()),
        "say hello",
    )
    .await;

    let Event::Error(message) = outcome else {
        panic!("a run that lost part of its record must not look completed: {outcome:?}");
    };
    assert!(
        message.contains("record") && message.contains("reproducible"),
        "the failure must say the run is unreproducible: {message}"
    );
    let fault = recorder.fault().expect("the loss must be kept, not logged");
    assert!(fault.detail.contains("no space left"), "{fault}");

    // And the tape itself must refuse to pass for a whole run.
    let err = RunCassette::load(&path).unwrap_err();
    assert!(
        matches!(err, ReplayError::Incomplete { .. }),
        "a short cassette must refuse to load: {err}"
    );
}

// ---------------------------------------------------------------------
// Finding 2 — every decision that shaped the trajectory is on the tape
// ---------------------------------------------------------------------

/// Plan mode refuses a mutating call before it ever runs. The model is
/// handed the refusal and carries on from there, so the refusal is a
/// fact about the run — and the metrics must count it like any other
/// call the model made.
#[tokio::test]
async fn a_call_plan_mode_refused_is_on_the_tape_and_in_the_count() {
    let repo = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("notes.md"), "# notes\n").unwrap();

    let script = Script::default();
    script.push(format!(
        "{}{}{}",
        tool_call_delta(
            0,
            Some("call_write"),
            Some("write_file"),
            r#"{"path":"notes.md","content":"rewritten"}"#
        ),
        finish_json("tool_calls", 10, 5),
        done()
    ));
    script.push(format!(
        "{}{}{}",
        text_delta("understood, staying in plan mode."),
        finish_json("stop", 20, 5),
        done()
    ));
    let base = serve(script.clone()).await;

    let path = vault.path().join("planned.jsonl");
    let recorder = RunRecorder::recording(&path).unwrap();
    let taping = Arc::new(RecordingProvider::new(live(&base), Arc::clone(&recorder)));
    let outcome = turn(
        repo.path(),
        &base,
        taping,
        Some(Arc::clone(&recorder)),
        PermissionMode::Plan,
        Some(ResumeState::default()),
        "rewrite the notes",
    )
    .await;
    assert!(
        matches!(outcome, Event::TurnCompleted { .. }),
        "plan mode refuses the tool, not the turn: {outcome:?}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("notes.md")).unwrap(),
        "# notes\n",
        "plan mode must not have let the write through"
    );

    let cassette = RunCassette::load(&path).unwrap();
    let outcomes = cassette.tool_outcomes();
    assert_eq!(
        outcomes.len(),
        1,
        "the refused call must be recorded: {outcomes:?}"
    );
    assert_eq!(outcomes[0].name, "write_file");
    assert_eq!(outcomes[0].disposition, ToolDisposition::PlanRefused);
    assert!(!outcomes[0].disposition.ran());
    assert!(!outcomes[0].ok);

    let metrics = cassette.metrics().unwrap();
    assert_eq!(
        metrics.tool_calls as usize,
        outcomes.len(),
        "the count and the tape must agree about what the model asked for"
    );
    assert_eq!(metrics.outcome, "completed");
}
