//! Verification-runner tests.
//!
//! Split out of `verify.rs` so the runner file stays about the rules;
//! these exercise every way a check can fail to prove something —
//! including the ways that look like silence.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::governance::manifest::Verdict;

const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";
const MANIFEST: &str = "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";

/// A dependency-free cargo project, so `cargo check` is a real verdict
/// that needs no network and compiles in well under a second.
fn cargo_fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), LIB).unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), MANIFEST).unwrap();
    tmp
}

fn accept(command: &str) -> Vec<AcceptanceCommand> {
    vec![AcceptanceCommand {
        command: command.into(),
        description: "acceptance".into(),
    }]
}

fn plan(scope: &[&str], mutated: &[&str], acceptance: Vec<AcceptanceCommand>) -> VerificationPlan {
    VerificationPlan {
        work_order_id: "wo-1".into(),
        goal: "make parse fallible".into(),
        scope: scope.iter().map(PathBuf::from).collect(),
        mutated: mutated.iter().map(PathBuf::from).collect(),
        witnesses: Vec::new(),
        acceptance,
    }
}

fn witness(root: &std::path::Path, rel: &str) -> ReadWitness {
    let bytes = std::fs::read(root.join(rel)).unwrap();
    ReadWitness {
        path: PathBuf::from(rel),
        file_hash: BlobHandle::of(&bytes).to_string(),
    }
}

fn check<'a>(m: &'a VerificationManifest, name: &str) -> &'a CheckOutcome {
    m.checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no {name} check in {:?}", m.checks))
}

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

/// A change to a file the run read but never declared writable never
/// passed the mutation gate — so verification is where it must surface.
#[tokio::test]
async fn a_change_outside_the_declared_scope_blocks_even_when_everything_compiles() {
    let tmp = cargo_fixture();
    let witnesses = vec![
        witness(tmp.path(), "src/lib.rs"),
        witness(tmp.path(), "Cargo.toml"),
    ];
    std::fs::write(tmp.path().join("src/lib.rs"), format!("{LIB}// snuck in\n")).unwrap();

    let mut p = plan(&["Cargo.toml"], &["Cargo.toml"], accept("cargo check"));
    p.witnesses = witnesses;
    let manifest = VerificationRunner::new(tmp.path()).run(&p).await;

    let Verdict::Blocked(reason) = manifest.verdict() else {
        panic!(
            "an unauthorized change cannot complete: {}",
            manifest.summary()
        );
    };
    assert!(reason.contains("src/lib.rs"), "{reason}");
    assert_eq!(manifest.breaches.len(), 1, "{:?}", manifest.breaches);
}

#[tokio::test]
async fn a_mutation_of_an_undeclared_path_is_a_breach() {
    let tmp = cargo_fixture();
    let manifest = VerificationRunner::new(tmp.path())
        .run(&plan(
            &["Cargo.toml"],
            &["Cargo.toml", "src/lib.rs"],
            accept("cargo check"),
        ))
        .await;
    assert_eq!(
        manifest.breaches.first().map(|b| b.path.clone()),
        Some(PathBuf::from("src/lib.rs"))
    );
}

/// Reading a file and leaving it alone is the normal case and must not
/// look like tampering.
#[tokio::test]
async fn untouched_witnesses_outside_the_scope_are_not_breaches() {
    let tmp = cargo_fixture();
    let mut p = plan(&["Cargo.toml"], &["Cargo.toml"], accept("cargo check"));
    p.witnesses = vec![witness(tmp.path(), "src/lib.rs")];
    let manifest = VerificationRunner::new(tmp.path()).run(&p).await;
    assert!(manifest.breaches.is_empty(), "{:?}", manifest.breaches);
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
