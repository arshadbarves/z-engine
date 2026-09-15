mod support;

use support::Fixture;

#[test]
fn ignored_and_generated_directories_are_pruned() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[workspace]"),
        (".gitignore", "ignored/\n*.tmp\n"),
        (".ignore", "private/\n"),
        ("ignored/package.json", "{bad"),
        ("private/package.json", "{bad"),
        (".git/package.json", "{bad"),
        ("node_modules/package.json", "{bad"),
        ("target/Cargo.toml", "bad"),
        ("vendor/go.mod", "bad"),
        ("build/pom.xml", "bad"),
        ("obj/app.csproj", "bad"),
        (".venv/pyproject.toml", "bad"),
        ("src/.gitignore", "generated/\n"),
        ("src/generated/package.json", "{bad"),
        ("src/real/package.json", r#"{"scripts":{"check":"check"}}"#),
    ]);
    let report = fixture.discover();
    assert!(!report.has_errors());
    assert_eq!(report.profiles.len(), 2);
    assert!(report.scan.skipped_ignored >= 10);
}

#[test]
fn ignore_rules_above_workspace_are_not_loaded() {
    let fixture = Fixture::new(&[
        (".gitignore", "Cargo.toml\n"),
        ("workspace/Cargo.toml", "[workspace]"),
    ]);
    let report =
        z_engine_project::discover(&fixture.path().join("workspace"), &Default::default()).unwrap();
    assert_eq!(
        report.profiles[0].manifests,
        [fixture.root().join("workspace/Cargo.toml")]
    );
}

#[cfg(unix)]
#[test]
fn symlinked_directories_manifests_and_ignore_files_are_not_followed() {
    use std::os::unix::fs::symlink;
    let outside = Fixture::new(&[
        ("package.json", "{outside malformed"),
        (".gitignore", "Cargo.toml\n"),
    ]);
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[workspace]"),
        ("inside/Makefile", "test:\n"),
    ]);
    symlink(outside.path(), fixture.path().join("external")).unwrap();
    symlink(
        outside.path().join("package.json"),
        fixture.path().join("package.json"),
    )
    .unwrap();
    symlink(
        outside.path().join(".gitignore"),
        fixture.path().join(".gitignore"),
    )
    .unwrap();
    symlink(fixture.path().join("inside"), fixture.path().join("alias")).unwrap();
    let report = fixture.discover();
    assert!(!report.has_errors());
    assert_eq!(report.profiles.len(), 2);
    assert_eq!(report.scan.skipped_symlinks, 4);
    assert!(
        report
            .profiles
            .iter()
            .flat_map(|p| &p.manifests)
            .all(|p| p.starts_with(fixture.root()))
    );
}

#[test]
fn manifest_references_do_not_expand_the_workspace() {
    let outside = Fixture::new(&[("Cargo.toml", "[package]\nname='outside'")]);
    let fixture = Fixture::new(&[(
        "Cargo.toml",
        &format!("[workspace]\nmembers = [{:?}]", outside.path()),
    )]);
    let report = fixture.discover();
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].root, fixture.root());
}

#[test]
fn unreadable_ignore_rules_do_not_disable_exclusions_silently() {
    let fixture = Fixture::new(&[
        (".gitignore", &"ignored/\n".repeat(100)),
        ("ignored/package.json", "{bad"),
    ]);
    let options = z_engine_project::DiscoveryOptions {
        max_manifest_bytes: 64,
        ..Default::default()
    };
    let report = z_engine_project::discover(fixture.path(), &options).unwrap();
    assert!(!report.scan.complete);
    assert_eq!(report.scan.bytes_read, 0);
    assert!(report.diagnostics.iter().any(|d| d.code == "byte_limit"));
    assert!(report.profiles.iter().all(|p| p.manifests.is_empty()));
}
