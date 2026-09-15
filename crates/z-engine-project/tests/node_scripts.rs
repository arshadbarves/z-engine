mod support;

use support::Fixture;
use z_engine_project::ProjectKind;

#[test]
fn lockfiles_select_the_actual_package_manager() {
    for (lock, expected) in [
        ("package-lock.json", "npm"),
        ("npm-shrinkwrap.json", "npm"),
        ("yarn.lock", "yarn"),
        ("pnpm-lock.yaml", "pnpm"),
        ("bun.lock", "bun"),
        ("bun.lockb", "bun"),
    ] {
        let fixture = Fixture::new(&[
            (
                "package.json",
                r#"{"scripts":{"test:unit":"test-runner","build":"builder","start":"server"}}"#,
            ),
            (lock, ""),
        ]);
        let report = fixture.discover();
        assert_eq!(report.profiles[0].commands.len(), 2);
        for command in &report.profiles[0].commands {
            assert_eq!(command.program, expected);
            assert_eq!(command.args[0], "run");
            assert!(command.evidence.contains(&fixture.root().join(lock)));
        }
    }
}

#[test]
fn nested_packages_inherit_manager_without_changing_cwd() {
    let fixture = Fixture::new(&[
        ("package.json", r#"{"packageManager":"pnpm@10.0.0"}"#),
        ("pnpm-lock.yaml", ""),
        (
            "packages/frontend/package.json",
            r#"{"scripts":{"test":"vitest"}}"#,
        ),
        (
            "packages/other/package.json",
            r#"{"packageManager":"bun@1.2.0","scripts":{"test":"bun test"}}"#,
        ),
    ]);
    let report = fixture.discover();
    let commands: Vec<_> = report.profiles.iter().flat_map(|p| &p.commands).collect();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].program, "pnpm");
    assert_eq!(commands[0].cwd, fixture.root().join("packages/frontend"));
    assert_eq!(commands[1].program, "bun");
}

#[test]
fn declaration_overrides_conflict_but_ambiguous_locks_do_not_guess() {
    let fixture = Fixture::new(&[
        ("package.json", r#"{"scripts":{"test":"vitest"}}"#),
        ("yarn.lock", ""),
        ("pnpm-lock.yaml", ""),
    ]);
    let report = fixture.discover();
    assert!(report.profiles[0].commands.is_empty());
    assert!(
        report.profiles[0]
            .diagnostics
            .iter()
            .any(|d| d.code == "ambiguous_package_manager")
    );
    fixture.write(
        "package.json",
        r#"{"packageManager":"yarn@4.0.0","scripts":{"test":"vitest"}}"#,
    );
    let report = fixture.discover();
    assert_eq!(report.profiles[0].commands[0].program, "yarn");
    assert!(
        report.profiles[0]
            .diagnostics
            .iter()
            .any(|d| d.code == "conflicting_lockfiles")
    );
}

#[test]
fn unknown_manager_and_placeholder_scripts_are_explicit() {
    let fixture = Fixture::new(&[(
        "package.json",
        r#"{"packageManager":"custom@1","scripts":{"test":"runner"}}"#,
    )]);
    let report = fixture.discover();
    assert!(report.profiles[0].commands.is_empty());
    assert!(
        report.profiles[0]
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_package_manager")
    );
    fixture.write("package.json", r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1","lint:fix":"eslint --fix .","check":"","dev":"vite"}}"#);
    let report = fixture.discover();
    assert_eq!(report.profiles[0].kind, ProjectKind::Node);
    assert!(report.profiles[0].commands.is_empty());
    assert!(
        report.profiles[0]
            .diagnostics
            .iter()
            .any(|d| d.code == "placeholder_script")
    );
}

#[test]
fn script_text_is_never_executed_and_no_script_is_invented() {
    let fixture = Fixture::new(&[(
        "package.json",
        r#"{"scripts":{"test":"touch DISCOVERY_MUST_NOT_EXECUTE","typecheck":"tsc","format:check":"formatter --check","format":"formatter --write"}}"#,
    )]);
    let report = fixture.discover();
    let scripts: Vec<_> = report.profiles[0]
        .commands
        .iter()
        .map(|c| c.args[1].as_str())
        .collect();
    assert_eq!(scripts, ["format:check", "test", "typecheck"]);
    assert!(!fixture.path().join("DISCOVERY_MUST_NOT_EXECUTE").exists());
}
