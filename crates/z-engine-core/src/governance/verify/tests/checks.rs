//! Check-execution tests: every way a check can fail to prove something,
//! including the ways that look like silence.

use super::*;

#[tokio::test]
async fn a_workspace_that_still_compiles_with_a_passing_acceptance_is_complete() {
    let tmp = cargo_fixture();
    let manifest = VerificationRunner::new(tmp.path())
        .run(&plan(
            &["src/lib.rs"],
            &["src/lib.rs"],
            accept("cargo check"),
        ))
        .await;
    assert_eq!(
        manifest.verdict(),
        Verdict::Complete,
        "{}",
        manifest.summary()
    );
    assert_eq!(check(&manifest, "cargo-check").status, CheckStatus::Passed);
}

/// The false-completion case at the unit level: the edit broke the build,
/// so no amount of confident prose can complete the run.
#[tokio::test]
async fn a_broken_edit_blocks_and_the_refusal_carries_the_compiler_error() {
    let tmp = cargo_fixture();
    std::fs::write(
        tmp.path().join("src/lib.rs"),
        "pub fn parse() -> usize { nope }\n",
    )
    .unwrap();

    let manifest = VerificationRunner::new(tmp.path())
        .run(&plan(
            &["src/lib.rs"],
            &["src/lib.rs"],
            accept("cargo check"),
        ))
        .await;

    let Verdict::Blocked(reason) = manifest.verdict() else {
        panic!("a broken build cannot complete: {}", manifest.summary());
    };
    assert!(reason.contains("cargo check"), "{reason}");
    let outcome = check(&manifest, "cargo-check");
    assert!(matches!(outcome.status, CheckStatus::Failed { .. }));
    assert!(
        outcome.output_tail.contains("src/lib.rs"),
        "the failure must name the file: {}",
        outcome.output_tail
    );
}

/// A manifest error produces *no* compiler diagnostics at all. The exit
/// status has to be authoritative or this reads as a clean run.
#[tokio::test]
async fn a_broken_manifest_blocks_even_though_cargo_emits_no_diagnostics() {
    let tmp = cargo_fixture();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package\nname = \"fixture\"\n",
    )
    .unwrap();

    let manifest = VerificationRunner::new(tmp.path())
        .run(&plan(
            &["Cargo.toml"],
            &["Cargo.toml"],
            accept("cargo check"),
        ))
        .await;

    assert!(!manifest.is_complete(), "{}", manifest.summary());
    assert!(matches!(
        check(&manifest, "cargo-check").status,
        CheckStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn a_hanging_acceptance_command_times_out_and_blocks() {
    let tmp = cargo_fixture();
    let started = Instant::now();
    let manifest = VerificationRunner::new(tmp.path())
        .with_timeout(Duration::from_millis(300))
        .with_allowed_programs(&["sleep"])
        .run(&plan(&["src/lib.rs"], &["src/lib.rs"], accept("sleep 120")))
        .await;

    assert_eq!(
        check(&manifest, "acceptance").status,
        CheckStatus::TimedOut { after_secs: 1 }
    );
    assert!(!manifest.is_complete(), "{}", manifest.summary());
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the bound must not wait out the child"
    );
}

#[tokio::test]
async fn an_acceptance_command_whose_program_is_missing_blocks() {
    let tmp = cargo_fixture();
    let manifest = VerificationRunner::new(tmp.path())
        .with_allowed_programs(&["z-engine-no-such-program"])
        .run(&plan(
            &["src/lib.rs"],
            &["src/lib.rs"],
            accept("z-engine-no-such-program --check"),
        ))
        .await;

    let CheckStatus::Unavailable { reason } = &check(&manifest, "acceptance").status else {
        panic!("a missing program proves nothing: {}", manifest.summary());
    };
    assert!(reason.contains("z-engine-no-such-program"), "{reason}");
    assert!(!manifest.is_complete());
}

#[tokio::test]
async fn an_acceptance_command_outside_the_allowlist_is_refused_unrun() {
    let tmp = cargo_fixture();
    let marker = tmp.path().join("ran");
    let manifest = VerificationRunner::new(tmp.path())
        .run(&plan(
            &["src/lib.rs"],
            &["src/lib.rs"],
            accept(&format!("touch {}", marker.display())),
        ))
        .await;

    assert!(matches!(
        check(&manifest, "acceptance").status,
        CheckStatus::Rejected { .. }
    ));
    assert!(!marker.exists());
    assert!(!manifest.is_complete());
}

#[tokio::test]
async fn a_mutating_order_with_no_acceptance_command_cannot_complete() {
    let tmp = cargo_fixture();
    let manifest = VerificationRunner::new(tmp.path())
        .run(&plan(&["src/lib.rs"], &["src/lib.rs"], vec![]))
        .await;

    let Verdict::Blocked(reason) = manifest.verdict() else {
        panic!("an unproven goal cannot complete: {}", manifest.summary());
    };
    assert!(reason.contains("no acceptance command"), "{reason}");
}

/// A project with no cargo manifest has nothing to compile; that is a
/// recorded skip, not an invisible pass.
#[tokio::test]
async fn a_project_without_a_cargo_manifest_records_the_skip() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("notes.md"), "# notes\n").unwrap();
    let manifest = VerificationRunner::new(tmp.path())
        .with_allowed_programs(&["true"])
        .run(&plan(&["notes.md"], &["notes.md"], accept("true")))
        .await;

    let outcome = check(&manifest, "cargo-check");
    assert!(!outcome.required);
    assert!(matches!(outcome.status, CheckStatus::Skipped { .. }));
    assert_eq!(
        manifest.verdict(),
        Verdict::Complete,
        "{}",
        manifest.summary()
    );
}

/// The hole a skipped `cargo check` would leave: with no harness-chosen
/// required check, the only required evidence is the order's own
/// acceptance command — the model grading its own work. Changing Rust
/// with no manifest to compile it against must therefore block.
#[tokio::test]
async fn changing_rust_with_no_manifest_to_compile_it_is_refused_not_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), LIB).unwrap();

    let manifest = VerificationRunner::new(tmp.path())
        .with_allowed_programs(&["true"])
        .run(&plan(&["src/lib.rs"], &["src/lib.rs"], accept("true")))
        .await;

    let outcome = check(&manifest, "cargo-check");
    assert!(outcome.required, "{outcome:?}");
    assert!(
        matches!(outcome.status, CheckStatus::Unavailable { .. }),
        "{outcome:?}"
    );
    assert!(
        matches!(manifest.verdict(), Verdict::Blocked(r) if r.contains("no Cargo.toml was found")),
        "a passing `true` must not stand in for compiling: {}",
        manifest.summary()
    );
}

/// A crate nested under a non-cargo root is still verifiable: the check
/// runs at the nearest enclosing manifest rather than being skipped.
#[tokio::test]
async fn a_crate_below_a_non_cargo_root_is_compiled_where_its_manifest_lives() {
    let tmp = tempfile::tempdir().unwrap();
    let crate_dir = tmp.path().join("rust/fixture");
    std::fs::create_dir_all(crate_dir.join("src")).unwrap();
    std::fs::write(crate_dir.join("Cargo.toml"), MANIFEST).unwrap();
    std::fs::write(
        crate_dir.join("src/lib.rs"),
        "pub fn parse() -> usize { \"x\" }\n",
    )
    .unwrap();

    let rel = "rust/fixture/src/lib.rs";
    let manifest = VerificationRunner::new(tmp.path())
        .with_allowed_programs(&["cargo"])
        .run(&plan(&[rel], &[rel], Vec::new()))
        .await;

    let outcome = check(&manifest, "cargo-check");
    assert!(outcome.required);
    assert!(
        matches!(outcome.status, CheckStatus::Failed { .. }),
        "the nested crate really has to be compiled: {outcome:?}"
    );
    assert!(
        outcome.output_tail.contains("mismatched types"),
        "{outcome:?}"
    );
}

/// Stopping the turn stops the checks: an abort must not have to wait out
/// a timeout that could be ten minutes long.
#[cfg(unix)]
#[tokio::test]
async fn an_aborted_run_stops_its_checks_instead_of_waiting_out_the_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let setter = std::sync::Arc::clone(&flag);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        setter.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let started = Instant::now();
    let manifest = VerificationRunner::new(tmp.path())
        .with_timeout(Duration::from_secs(600))
        .with_allowed_programs(&["sleep"])
        .with_abort(flag)
        .run(&plan(&["notes.md"], &["notes.md"], accept("sleep 120")))
        .await;

    assert!(
        started.elapsed() < Duration::from_secs(20),
        "an abort must not wait out the timeout"
    );
    let outcome = check(&manifest, "acceptance");
    assert!(
        matches!(&outcome.status, CheckStatus::Unavailable { reason } if reason.contains("stopped")),
        "a stopped check proves nothing: {outcome:?}"
    );
    assert!(matches!(manifest.verdict(), Verdict::Blocked(_)));
}

#[test]
fn the_manifest_is_written_where_the_refusal_can_point_at_it() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runs/01ABC");
    let manifest = VerificationManifest {
        work_order_id: "wo-1".into(),
        goal: "g".into(),
        scope: vec![PathBuf::from("src/lib.rs")],
        mutated: vec![],
        breaches: vec![],
        checks: vec![],
    };
    let path = write_manifest(&dir, &manifest).unwrap();
    assert_eq!(path, dir.join("verification.json"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        serde_json::from_str::<VerificationManifest>(&text).unwrap(),
        manifest
    );
}
