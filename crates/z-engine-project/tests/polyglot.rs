mod support;

use support::Fixture;
use z_engine_project::{
    CapabilityStatus, ExecutionStatus, ProjectKind, SupportLevel, VerificationKind,
};

#[test]
fn root_and_nested_projects_have_exact_cwds_and_unexecuted_commands() {
    let fixture = Fixture::new(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = ['rust', '../outside']",
        ),
        (
            "rust/Cargo.toml",
            "[package]\nname='inner'\nversion='0.1.0'",
        ),
        (
            "web/package.json",
            r#"{"scripts":{"test":"vitest run","typecheck":"tsc --noEmit","dev":"vite"}}"#,
        ),
        ("web/pnpm-lock.yaml", ""),
        ("web/src/main.ts", ""),
        (
            "python/pyproject.toml",
            "[project]\nname='example'\n[tool.pytest.ini_options]\n[tool.ruff]",
        ),
        ("go/go.mod", "module example.com/service\n\ngo 1.23\n"),
        (
            "jvm/pom.xml",
            "<project><modelVersion>4.0.0</modelVersion><artifactId>demo</artifactId></project>",
        ),
        ("jvm/src/Main.java", ""),
        ("gradle/build.gradle.kts", "plugins { java }"),
        (
            "dotnet/app.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><IsTestProject>true</IsTestProject></PropertyGroup></Project>"#,
        ),
        ("native/CMakeLists.txt", "project(example)"),
        (
            "native/Makefile",
            "build:\n\techo build\ncheck:\n\techo check\n",
        ),
    ]);
    let report = fixture.discover();
    assert!(report.scan.complete);
    assert!(!report.has_errors());
    assert_eq!(report.profiles.len(), 10);
    assert_eq!(report.execution, ExecutionStatus::NotRun);
    for profile in &report.profiles {
        assert!(profile.root.starts_with(fixture.root()));
        assert_eq!(
            profile.capabilities.toolchain_availability,
            CapabilityStatus::Unknown
        );
        assert_eq!(
            profile.capabilities.semantic_refactoring,
            CapabilityStatus::Unknown
        );
        assert_eq!(
            profile.capabilities.verified_completion,
            CapabilityStatus::NotProvided
        );
        for command in &profile.commands {
            assert_eq!(command.cwd, profile.root);
            assert_eq!(command.execution, ExecutionStatus::NotRun);
            assert!(
                command
                    .evidence
                    .iter()
                    .all(|path| path.starts_with(fixture.root()) && path.is_file())
            );
        }
    }
    let web = report
        .profiles
        .iter()
        .find(|p| p.kind == ProjectKind::Node)
        .unwrap();
    assert_eq!(web.root, fixture.root().join("web"));
    assert_eq!(web.languages, ["TypeScript"]);
    assert_eq!(web.commands.len(), 2);
    assert!(web.commands.iter().all(|c| c.program == "pnpm"));
    let dotnet = report
        .profiles
        .iter()
        .find(|p| p.kind == ProjectKind::Dotnet)
        .unwrap();
    assert!(
        dotnet
            .commands
            .iter()
            .any(|c| c.kind == VerificationKind::Test && c.args == ["test", "app.csproj"])
    );
    let gradle = report
        .profiles
        .iter()
        .find(|p| p.kind == ProjectKind::Gradle)
        .unwrap();
    assert_eq!(gradle.support, SupportLevel::MarkerOnly);
    assert!(gradle.commands.is_empty());
}

#[test]
fn unsupported_and_empty_workspaces_are_not_verified() {
    for files in [vec![("main.rb", "puts 'hello'"), ("Gemfile", "")], vec![]] {
        let fixture = Fixture::new(&files);
        let report = fixture.discover();
        assert_eq!(report.profiles.len(), 1);
        let profile = &report.profiles[0];
        assert_eq!(profile.kind, ProjectKind::Unknown);
        assert_eq!(profile.support, SupportLevel::Unsupported);
        assert_eq!(profile.capabilities.test, CapabilityStatus::Unconfigured);
        assert_eq!(
            profile.capabilities.verified_completion,
            CapabilityStatus::NotProvided
        );
        assert!(profile.commands.is_empty());
        if !files.is_empty() {
            assert_eq!(report.languages, ["Ruby"]);
        }
    }
}

#[test]
fn python_does_not_invent_a_test_runner_from_dependencies_or_source() {
    let fixture = Fixture::new(&[
        (
            "pyproject.toml",
            "[project]\nname='example'\ndependencies=['requests']\n[build-system]\nrequires=['setuptools']\nbuild-backend='setuptools.build_meta'",
        ),
        ("test_example.py", "def test_example(): pass"),
    ]);
    let report = fixture.discover();
    assert!(report.profiles[0].commands.is_empty());
    assert_eq!(
        report.profiles[0].capabilities.test,
        CapabilityStatus::Unconfigured
    );
}
