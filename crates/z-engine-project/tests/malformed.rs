mod support;

use support::Fixture;

#[test]
fn malformed_manifests_are_actionable_per_profile_errors() {
    for (file, text) in [
        ("Cargo.toml", "[bad"),
        ("Cargo.toml", "package = []"),
        ("Cargo.toml", "[package]\nname=123"),
        ("package.json", "{bad"),
        ("package.json", "[]"),
        ("package.json", "null"),
        ("package.json", r#"{"scripts":{"test":123}}"#),
        ("package.json", r#"{"packageManager":false}"#),
        ("pyproject.toml", "[tool.pytest.ini_options"),
        ("pyproject.toml", "tool = []"),
        ("pytest.ini", "[pytest"),
        ("pytest.ini", "testpaths = tests"),
        ("go.mod", "go 1.23"),
        ("go.mod", "module a\nmodule b"),
        ("pom.xml", "<project><missing></project>"),
        ("pom.xml", "<project/>"),
        ("app.csproj", "<Project><PropertyGroup>"),
        ("app.csproj", "<Wrong/>"),
        ("app.sln", "not a solution"),
        ("CMakePresets.json", r#"{"version":3,"buildPresets":"bad"}"#),
        ("CMakePresets.json", "[3, [], []]"),
    ] {
        let fixture = Fixture::new(&[(file, text)]);
        let report = fixture.discover();
        assert!(report.has_errors(), "expected error for {file}: {text}");
        assert!(report.profiles[0].commands.is_empty());
        let diagnostic = &report.profiles[0].diagnostics[0];
        assert_eq!(diagnostic.code, "malformed_manifest");
        assert!(diagnostic.message.contains("Fix this manifest"));
        assert_eq!(diagnostic.path, fixture.root().join(file));
    }
}

#[test]
fn a_bad_nested_manifest_does_not_hide_good_profiles() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[workspace]"),
        ("nested/package.json", "{bad"),
    ]);
    let report = fixture.discover();
    assert!(report.has_errors());
    assert_eq!(report.profiles.len(), 2);
    assert!(!report.profiles[0].commands.is_empty());
    assert!(report.profiles[1].commands.is_empty());
}

#[test]
fn external_xml_entities_are_never_resolved() {
    let fixture = Fixture::new(&[(
        "pom.xml",
        r#"<!DOCTYPE project SYSTEM "https://example.invalid/evil"><project/>"#,
    )]);
    let report = fixture.discover();
    assert!(report.has_errors());
    assert!(report.profiles[0].diagnostics[0].message.contains("DTD"));
}

#[test]
fn invalid_utf8_is_a_reported_read_error() {
    let fixture = Fixture::new(&[]);
    std::fs::write(fixture.path().join("package.json"), [0xff, 0xfe]).unwrap();
    let report = fixture.discover();
    assert!(report.has_errors());
    assert!(!report.scan.complete);
    assert_eq!(report.profiles[0].diagnostics[0].code, "invalid_encoding");
}
