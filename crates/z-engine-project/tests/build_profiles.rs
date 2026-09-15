mod support;

use support::Fixture;
use z_engine_project::{ProjectKind, VerificationKind};

#[test]
fn python_ini_formats_require_actual_pytest_sections() {
    for (name, section) in [
        ("pytest.ini", "pytest"),
        ("tox.ini", "pytest"),
        ("setup.cfg", "tool:pytest"),
    ] {
        let fixture = Fixture::new(&[(name, &format!("[{section}]\ntestpaths =\n    tests\n"))]);
        let report = fixture.discover();
        assert!(!report.has_errors());
        let command = &report.profiles[0].commands[0];
        assert_eq!(command.program, "python");
        assert_eq!(command.args, ["-m", "pytest"]);
    }
}

#[test]
fn cmake_uses_only_declared_resolvable_preset_names() {
    let fixture = Fixture::new(&[(
        "CMakePresets.json",
        r#"{
        "version": 3,
        "configurePresets": [{"name":"debug","generator":"Ninja","binaryDir":"${sourceDir}/build"}],
        "buildPresets": [
            {"name":"build-debug","configurePreset":"debug"},
            {"name":"hidden","hidden":true,"configurePreset":"debug"},
            {"name":"inherited","inherits":"outside"},
            {"name":"conditional","configurePreset":"debug","condition":false}
        ],
        "testPresets": [{"name":"test-debug","configurePreset":"debug"}]
    }"#,
    )]);
    let report = fixture.discover();
    assert!(!report.has_errors());
    let commands = &report.profiles[0].commands;
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].args, ["--build", "--preset", "build-debug"]);
    assert_eq!(commands[1].program, "ctest");
    assert_eq!(commands[1].args, ["--preset", "test-debug"]);
    fixture.write(
        "CMakePresets.json",
        r#"{"version":1,"configurePresets":[]}"#,
    );
    let report = fixture.discover();
    assert!(!report.has_errors());
    assert!(report.profiles[0].commands.is_empty());
}

#[test]
fn make_targets_are_literal_not_variables_recipes_or_conditionals() {
    let fixture = Fixture::new(&[(
        "Makefile",
        concat!(
            "VARIABLE = value \\\n",
            "test:\n",
            "  ifdef ENABLE_TESTS\n",
            "test:\n",
            "  endif\n",
            "define COMMANDS\n",
            "test:\n",
            "endef\n",
            "lint := not a target\n",
            "check:\n",
            "\tprintf 'test:\\n'\n",
            "build:\n",
            "\techo build\n",
        ),
    )]);
    let report = fixture.discover();
    assert_eq!(report.profiles[0].commands.len(), 2);
    assert!(
        report.profiles[0]
            .commands
            .iter()
            .all(|c| c.kind != VerificationKind::Test)
    );
    assert!(
        report.profiles[0]
            .commands
            .iter()
            .any(|c| c.args == ["-f", "Makefile", "check"])
    );
}

#[test]
fn maven_wrapper_is_suggested_only_when_present() {
    let wrapper = if cfg!(windows) { "mvnw.cmd" } else { "mvnw" };
    let fixture = Fixture::new(&[
        (
            "pom.xml",
            "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><artifactId>example</artifactId></project>",
        ),
        (wrapper, "echo never execute"),
    ]);
    let report = fixture.discover();
    assert!(
        report.profiles[0]
            .commands
            .iter()
            .all(|c| { c.program == fixture.root().join(wrapper).to_string_lossy() })
    );
}

#[test]
fn xml_escapes_do_not_suppress_maven_or_dotnet_capabilities() {
    let fixture = Fixture::new(&[
        (
            "pom.xml",
            "<project><modelVersion>4.0.0</modelVersion>\
         <artifactId>example</artifactId><description>A &amp; B</description></project>",
        ),
        (
            "tests.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup>\
         <IsTestProject> tr&#117;e </IsTestProject><Description>A &amp; B</Description>\
         </PropertyGroup></Project>",
        ),
    ]);
    let report = fixture.discover();
    assert!(!report.has_errors(), "{report:#?}");
    for kind in [ProjectKind::Maven, ProjectKind::Dotnet] {
        let profile = report
            .profiles
            .iter()
            .find(|profile| profile.kind == kind)
            .unwrap();
        assert!(
            profile
                .commands
                .iter()
                .any(|command| command.kind == VerificationKind::Test)
        );
    }
}

#[test]
fn dotnet_does_not_invent_tests_for_application_projects_or_solutions() {
    let fixture = Fixture::new(&[
        (
            "application.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk\" />",
        ),
        (
            "tests.fsproj",
            "<Project Sdk=\"Microsoft.NET.Sdk\"><ItemGroup><PackageReference Include=\"Microsoft.NET.Test.Sdk\" Version=\"17.0.0\" /></ItemGroup></Project>",
        ),
        (
            "example.sln",
            "Microsoft Visual Studio Solution File, Format Version 12.00\n",
        ),
        (
            "modern.slnx",
            "<Solution><Project Path=\"../outside/app.csproj\" /></Solution>",
        ),
    ]);
    let report = fixture.discover();
    assert!(!report.has_errors());
    assert_eq!(report.profiles[0].kind, ProjectKind::Dotnet);
    let tests: Vec<_> = report.profiles[0]
        .commands
        .iter()
        .filter(|c| c.kind == VerificationKind::Test)
        .collect();
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].args, ["test", "tests.fsproj"]);
    assert_eq!(
        report.profiles[0]
            .commands
            .iter()
            .filter(|c| c.kind == VerificationKind::Build)
            .count(),
        4
    );
}
