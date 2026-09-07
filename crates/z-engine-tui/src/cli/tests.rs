//! What each invocation means, and what it is refused for.

use super::*;

fn args(line: &str) -> Result<Args, CliError> {
    parse(&line.split(' ').map(str::to_string).collect::<Vec<_>>())
}

#[test]
fn the_guarded_flag_is_read_for_interactive_and_headless_runs_alike() {
    assert!(args("--guarded").unwrap().guarded);
    let headless = args("--guarded --headless fix the parser").unwrap();
    assert!(headless.guarded);
    assert_eq!(headless.headless_task.as_deref(), Some("fix the parser"));
    assert!(!args("--headless go").unwrap().guarded);
}

#[test]
fn a_cassette_path_is_read_for_recording_and_for_replay() {
    let recorded = args("--headless go --record-run /tapes/run.jsonl").unwrap();
    assert_eq!(recorded.record_run, Some(PathBuf::from("/tapes/run.jsonl")));
    let replayed = args("--headless go --replay-run /tapes/run.jsonl").unwrap();
    assert_eq!(replayed.replay_run, Some(PathBuf::from("/tapes/run.jsonl")));
}

#[test]
fn a_flag_without_its_value_is_named_in_the_refusal() {
    assert_eq!(
        args("--headless go --record-run"),
        Err(CliError::MissingValue("--record-run".into()))
    );
    assert_eq!(
        args("--metrics-out"),
        Err(CliError::MissingValue("--metrics-out".into()))
    );
}

#[test]
fn recording_a_replay_is_refused() {
    assert_eq!(
        args("--headless go --record-run /a.jsonl --replay-run /b.jsonl"),
        Err(CliError::RecordAndReplay)
    );
}

#[test]
fn taping_an_interactive_session_is_refused_and_says_what_to_do() {
    assert_eq!(
        args("--record-run /a.jsonl"),
        Err(CliError::NeedsHeadless("--record-run".into()))
    );
    assert_eq!(
        args("--replay-run /a.jsonl"),
        Err(CliError::NeedsHeadless("--replay-run".into()))
    );
    assert_eq!(
        args("--metrics-out /m.json"),
        Err(CliError::NeedsHeadless("--metrics-out".into()))
    );
    let told = args("--record-run /a.jsonl").unwrap_err().to_string();
    assert!(told.contains("--headless"), "{told}");
}

#[test]
fn metrics_without_a_tape_have_nothing_to_read() {
    assert_eq!(
        args("--headless go --metrics-out /m.json"),
        Err(CliError::MetricsWithoutTape)
    );
    assert!(
        args("--headless go --replay-run /a.jsonl --metrics-out /m.json")
            .unwrap()
            .metrics_out
            .is_some()
    );
}

#[test]
fn a_replay_cannot_be_pointed_at_another_model_or_host() {
    assert_eq!(
        args("--headless go --replay-run /a.jsonl --model other"),
        Err(CliError::ReplayOverride("--model".into()))
    );
    assert_eq!(
        args("--headless go --replay-run /a.jsonl --base-url http://x"),
        Err(CliError::ReplayOverride("--base-url".into()))
    );
}

#[test]
fn a_replayed_run_never_resolves_a_key_and_every_other_run_does() {
    let replayed = args("--headless go --replay-run /a.jsonl").unwrap();
    assert!(!replayed.needs_api_key());
    let recorded = args("--headless go --record-run /a.jsonl").unwrap();
    assert!(recorded.needs_api_key());
    assert!(args("--guarded").unwrap().needs_api_key());
}

#[test]
fn help_short_circuits_every_other_rule() {
    let asked = args("--record-run /a.jsonl --help").unwrap();
    assert!(asked.help);
    assert!(USAGE.contains("--guarded") && USAGE.contains("--replay-run"));
}

#[test]
fn an_unknown_argument_is_named() {
    assert_eq!(
        args("--guarded --nope"),
        Err(CliError::Unknown("--nope".into()))
    );
}

#[test]
fn a_tape_inside_the_project_is_refused() {
    let project = tempfile::tempdir().unwrap();
    let parsed = Args {
        headless_task: Some("go".into()),
        record_run: Some(project.path().join("run.jsonl")),
        ..Args::default()
    };
    let err = parsed.check_tape_paths(project.path()).unwrap_err();
    assert!(
        matches!(&err, CliError::TapeInsideProject { flag, .. } if flag == "--record-run"),
        "{err}"
    );
    assert!(err.to_string().contains("outside the project"), "{err}");
}

#[test]
fn a_tape_in_a_directory_the_run_would_create_inside_the_project_is_refused() {
    // The parent does not exist yet, and on macOS the project root is
    // reached through a symlink: the check has to survive both at once.
    let project = tempfile::tempdir().unwrap();
    let parsed = Args {
        headless_task: Some("go".into()),
        record_run: Some(project.path().join("tapes/run.jsonl")),
        ..Args::default()
    };
    assert!(
        parsed.check_tape_paths(project.path()).is_err(),
        "a tape under a directory the run would create is still inside the project"
    );
}

#[test]
fn a_tape_outside_the_project_is_accepted() {
    let project = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    let parsed = Args {
        headless_task: Some("go".into()),
        record_run: Some(vault.path().join("tapes/run.jsonl")),
        metrics_out: Some(vault.path().join("metrics.json")),
        ..Args::default()
    };
    assert!(parsed.check_tape_paths(project.path()).is_ok());
}
