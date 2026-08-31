//! Scope-audit tests: what the declared work order does and does not
//! account for, and who is blamed for a change on disk.

use super::*;

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

/// The checks are processes that write to the workspace themselves.
/// Auditing after they run would charge `cargo check`'s own refresh of
/// `Cargo.lock` to the agent as a scope breach it never committed.
#[tokio::test]
async fn the_verifier_s_own_writes_are_not_charged_to_the_agent() {
    let repo = cargo_fixture();
    // Read before it exists in its final form: `cargo check` will write
    // the lock file, changing it out from under this witness.
    std::fs::write(repo.path().join("Cargo.lock"), "# placeholder\n").unwrap();
    let mut p = plan(
        &["src/lib.rs"],
        &["src/lib.rs"],
        accept("cargo check --quiet"),
    );
    p.witnesses = vec![witness(repo.path(), "Cargo.lock")];

    let manifest = VerificationRunner::new(repo.path()).run(&p).await;

    assert!(
        manifest.breaches.is_empty(),
        "cargo's own lock-file refresh is not the agent's breach: {:?}",
        manifest.breaches
    );
    assert_eq!(
        manifest.verdict(),
        Verdict::Complete,
        "{}",
        manifest.summary()
    );
}
