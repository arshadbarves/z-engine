use super::process::execute;
use super::runner::parse_summary;
use super::test_workspace::TestWorkspace;
use super::*;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

#[test]
fn fixed_argv_and_input_validation() {
    let spec = CheckSpec {
        kind: CheckKind::CargoTest,
        package: None,
        filter: None,
    };
    assert_eq!(
        spec.command().unwrap(),
        ["cargo", "test", "--workspace", "--no-fail-fast"]
    );
    let build = CheckSpec {
        kind: CheckKind::CargoBuild,
        package: None,
        filter: None,
    };
    assert_eq!(
        build.command().unwrap(),
        ["cargo", "build", "--workspace", "--all-targets"]
    );
    for value in [
        serde_json::json!({"kind":"cargo_test","command":"rm -rf ."}),
        serde_json::json!({"kind":"cargo_test","package":"--all"}),
        serde_json::json!({"kind":"cargo_test","filter":"--ignored"}),
        serde_json::json!({"kind":"cargo_test","filter":"test;echo"}),
        serde_json::json!({"kind":"cargo_build","filter":"test"}),
    ] {
        assert!(
            serde_json::from_value::<CheckSpec>(value)
                .map_err(|e| e.to_string())
                .and_then(|spec| spec.validate().map_err(|e| e.to_string()))
                .is_err()
        );
    }
}

#[test]
fn unavailable_check_evidence_never_invents_process_results() {
    let spec = CheckSpec {
        kind: CheckKind::CargoTest,
        package: None,
        filter: None,
    };
    let evidence = blocked_evidence(
        std::path::Path::new("workspace"),
        spec,
        "storage unavailable",
    )
    .unwrap();
    assert_eq!(evidence.outcome, CheckOutcome::Blocked);
    assert_eq!(evidence.exit_code, None);
    assert_eq!(evidence.tests_run, None);
    assert_eq!(evidence.input_fingerprint, None);
    assert_eq!(evidence.stdout, None);
    assert_eq!(evidence.stderr, None);
    assert_eq!(evidence.summary, "storage unavailable");
    assert!(!evidence.id.is_empty());
}

#[test]
fn rust_summaries_count_executed_not_ignored_or_filtered() {
    assert_eq!(
        parse_summary(
            "test result: ok. 3 passed; 0 failed; 7 ignored; 0 measured; 2 filtered out; finished in 0.0s"
        ),
        Some((3, 0))
    );
    assert_eq!(
        parse_summary(
            "test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s"
        ),
        Some((5, 2))
    );
    assert_eq!(
        parse_summary(
            "test result: ok. 0 passed; 0 failed; 7 ignored; 0 measured; 2 filtered out; finished in 0.0s"
        ),
        Some((0, 0))
    );
    assert_eq!(parse_summary("all tests passed!"), None);
    assert_eq!(parse_summary("test result: ok. nope"), None);
}

async fn process(
    command: Vec<String>,
    abort: bool,
    timeout: Duration,
) -> super::process::ProcessResult {
    let root = TestWorkspace::new();
    let out = root.0.join("out.log");
    let err = root.0.join("err.log");
    std::fs::write(&out, "").unwrap();
    std::fs::write(&err, "").unwrap();
    execute(
        root.path(),
        &command,
        [&out, &err],
        Arc::new(AtomicBool::new(abort)),
        timeout,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn spawn_failure_and_pre_cancel_are_evidence() {
    let result = process(
        vec!["z-engine-nonexistent-verification-command".into()],
        false,
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(result.outcome, CheckOutcome::Blocked);
    assert_eq!(result.code, None);
    let result = process(vec!["cargo".into()], true, Duration::from_secs(1)).await;
    assert_eq!(result.outcome, CheckOutcome::Cancelled);
    assert_eq!(result.code, None);
}

#[cfg(unix)]
#[tokio::test]
async fn nonzero_without_diagnostics_and_timeout_are_not_success() {
    let result = process(
        vec!["sh".into(), "-c".into(), "exit 7".into()],
        false,
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(result.outcome, CheckOutcome::Failed);
    assert_eq!(result.code, Some(7));
    let result = process(
        vec!["sh".into(), "-c".into(), "sleep 20 & wait".into()],
        false,
        Duration::from_millis(100),
    )
    .await;
    assert_eq!(result.outcome, CheckOutcome::Blocked);
    assert!(result.message.contains("timed out"));
    assert_ne!(result.code, Some(0));
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_and_reaps_the_owned_process_group() {
    use std::sync::atomic::Ordering;
    let root = TestWorkspace::new();
    let out = root.0.join("out.log");
    let err = root.0.join("err.log");
    std::fs::write(&out, "").unwrap();
    std::fs::write(&err, "").unwrap();
    let abort = Arc::new(AtomicBool::new(false));
    let flag = abort.clone();
    let timer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        flag.store(true, Ordering::Relaxed);
    });
    let result = execute(
        root.path(),
        &["sh".into(), "-c".into(), "sleep 30 & echo $!; wait".into()],
        [&out, &err],
        abort,
        Duration::from_secs(3),
    )
    .await
    .unwrap();
    timer.await.unwrap();
    assert_eq!(result.outcome, CheckOutcome::Cancelled);
    let descendant: u32 = std::fs::read_to_string(out)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status = std::process::Command::new("ps")
        .args(["-p", &descendant.to_string(), "-o", "stat="])
        .output()
        .unwrap();
    let state = String::from_utf8(status.stdout).unwrap();
    assert!(
        state.trim().is_empty() || state.trim().starts_with('Z'),
        "descendant still running: {state}"
    );
}
