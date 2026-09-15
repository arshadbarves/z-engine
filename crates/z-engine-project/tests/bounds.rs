mod support;

use std::sync::atomic::AtomicBool;

use support::Fixture;
use z_engine_project::{DiscoveryError, DiscoveryOptions, discover, discover_with_cancel};

#[test]
fn depth_entry_and_profile_limits_are_visible() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[workspace]"),
        ("nested/package.json", "{}"),
        ("nested/deeper/go.mod", "module example.com/deeper"),
    ]);
    let options = DiscoveryOptions {
        max_depth: 0,
        ..Default::default()
    };
    let report = discover(fixture.path(), &options).unwrap();
    assert!(!report.scan.complete);
    assert_eq!(report.profiles.len(), 1);
    assert!(report.diagnostics.iter().any(|d| d.code == "depth_limit"));
    let options = DiscoveryOptions {
        max_entries: 1,
        ..Default::default()
    };
    let report = discover(fixture.path(), &options).unwrap();
    assert!(!report.scan.complete);
    assert!(report.scan.entries_seen <= 1);
    assert!(report.diagnostics.iter().any(|d| d.code == "entry_limit"));
    let options = DiscoveryOptions {
        max_profiles: 1,
        ..Default::default()
    };
    let report = discover(fixture.path(), &options).unwrap();
    assert_eq!(report.profiles.len(), 1);
    assert!(report.diagnostics.iter().any(|d| d.code == "profile_limit"));
}

#[test]
fn byte_limits_include_ignore_files_and_do_not_read_oversized_input() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[workspace]"),
        (".gitignore", "#1234567890"),
        ("nested/package.json", &" ".repeat(100)),
    ]);
    let options = DiscoveryOptions {
        max_manifest_bytes: 20,
        max_total_bytes: 22,
        ..Default::default()
    };
    let report = discover(fixture.path(), &options).unwrap();
    assert!(!report.scan.complete);
    assert!(report.scan.bytes_read <= 22);
    assert_eq!(report.scan.bytes_read, 22);
    assert!(
        report
            .profiles
            .iter()
            .flat_map(|p| &p.diagnostics)
            .any(|d| d.code == "byte_limit")
    );
}

#[test]
fn cancellation_invalid_limits_and_bad_workspace_are_typed_errors() {
    let fixture = Fixture::new(&[("file", "")]);
    assert!(matches!(
        discover_with_cancel(fixture.path(), &Default::default(), &AtomicBool::new(true)),
        Err(DiscoveryError::Cancelled)
    ));
    assert!(matches!(
        discover(
            fixture.path(),
            &DiscoveryOptions {
                max_depth: 17,
                ..Default::default()
            }
        ),
        Err(DiscoveryError::InvalidOptions(_))
    ));
    assert!(matches!(
        discover(&fixture.path().join("file"), &Default::default()),
        Err(DiscoveryError::NotDirectory(_))
    ));
    assert!(matches!(
        discover(&fixture.path().join("missing"), &Default::default()),
        Err(DiscoveryError::Workspace { .. })
    ));
}

#[test]
fn commands_are_bounded_even_for_many_declared_scripts() {
    let scripts: serde_json::Map<_, _> = (0..100)
        .map(|i| {
            (
                format!("test:{i:03}"),
                serde_json::Value::String("test-runner".into()),
            )
        })
        .collect();
    let fixture = Fixture::new(&[(
        "package.json",
        &serde_json::json!({"scripts": scripts}).to_string(),
    )]);
    let report = fixture.discover();
    assert_eq!(report.profiles[0].commands.len(), 32);
    assert!(
        report.profiles[0]
            .diagnostics
            .iter()
            .any(|d| d.code == "command_limit")
    );
}

#[test]
fn manifests_and_unsupported_profile_overflow_are_visible() {
    let fixture = Fixture::new(&[]);
    for index in 0..40 {
        fixture.write(
            &format!("project{index:02}.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\" />",
        );
    }
    let report = fixture.discover();
    assert!(!report.scan.complete);
    assert_eq!(report.profiles[0].manifests.len(), 32);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "manifest_limit")
    );
    let fixture = Fixture::new(&[("nested/Cargo.toml", "[workspace]"), ("unknown.rb", "")]);
    let options = DiscoveryOptions {
        max_profiles: 1,
        ..Default::default()
    };
    let report = discover(fixture.path(), &options).unwrap();
    assert!(!report.scan.complete);
    assert!(report.languages.contains(&"Ruby".into()));
    assert!(report.diagnostics.iter().any(|d| d.code == "profile_limit"));
}
