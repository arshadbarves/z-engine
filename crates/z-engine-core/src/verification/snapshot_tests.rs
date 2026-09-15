use super::test_workspace::TestWorkspace;
use super::*;

#[test]
fn deterministic_content_fingerprint_tracks_add_edit_delete_and_config() {
    let root = TestWorkspace::new();
    let initial = WorkspaceSnapshot::capture(root.path()).unwrap();
    assert_eq!(initial, WorkspaceSnapshot::capture(root.path()).unwrap());
    std::fs::write(root.0.join("src/lib.rs"), "pub fn value() -> u32 { 2 }\n").unwrap();
    std::fs::write(root.0.join("Cargo.lock"), "new lockfile").unwrap();
    let edited = WorkspaceSnapshot::capture(root.path()).unwrap();
    assert_ne!(initial.fingerprint, edited.fingerprint);
    assert_eq!(initial.changed_paths(&edited), ["Cargo.lock", "src/lib.rs"]);
    std::fs::remove_file(root.0.join("src/lib.rs")).unwrap();
    assert_eq!(
        edited.changed_paths(&WorkspaceSnapshot::capture(root.path()).unwrap()),
        ["src/lib.rs"]
    );
}

#[test]
fn generated_files_do_not_invalidate_inputs() {
    let root = TestWorkspace::new();
    let before = WorkspaceSnapshot::capture(root.path()).unwrap();
    for directory in ["target", "node_modules", ".z-engine"] {
        std::fs::create_dir_all(root.0.join(directory)).unwrap();
        std::fs::write(root.0.join(directory).join("output"), "generated").unwrap();
    }
    assert_eq!(before, WorkspaceSnapshot::capture(root.path()).unwrap());
}

#[test]
fn external_path_dependency_and_unsupported_harness_are_rejected() {
    for suffix in [
        "[dependencies]\noutside = { path = \"../outside\" }\n",
        "[[test]]\nname = \"custom\"\nharness = false\n",
    ] {
        let root = TestWorkspace::new();
        let path = root.0.join("Cargo.toml");
        let mut manifest = std::fs::read_to_string(&path).unwrap();
        manifest.push_str(suffix);
        std::fs::write(path, manifest).unwrap();
        assert!(matches!(
            WorkspaceSnapshot::capture(root.path()),
            Err(VerificationError::Unsupported(_))
        ));
    }
}

#[cfg(unix)]
#[test]
fn symlinked_sources_are_rejected_even_when_ignored() {
    let root = TestWorkspace::new();
    std::fs::write(root.0.join(".gitignore"), "link.rs\n").unwrap();
    std::os::unix::fs::symlink("src/lib.rs", root.0.join("link.rs")).unwrap();
    assert!(matches!(
        WorkspaceSnapshot::capture(root.path()),
        Err(VerificationError::Unsupported(_))
    ));
}

#[test]
fn oversized_source_is_explicit_error() {
    let root = TestWorkspace::new();
    std::fs::File::create(root.0.join("large.bin"))
        .unwrap()
        .set_len(33 * 1024 * 1024)
        .unwrap();
    assert!(matches!(
        WorkspaceSnapshot::capture(root.path()),
        Err(VerificationError::ScanLimit(_))
    ));
}

#[test]
fn git_manifest_includes_dirty_untracked_and_ignored_rust_inputs() {
    let root = TestWorkspace::new();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("git")
            .args(["add", "Cargo.toml", "src/lib.rs"])
            .current_dir(root.path())
            .status()
            .unwrap()
            .success()
    );
    let before = WorkspaceSnapshot::capture(root.path()).unwrap();
    std::fs::write(root.0.join("src/lib.rs"), "pub fn value() -> u32 { 5 }\n").unwrap();
    std::fs::write(root.0.join(".gitignore"), "ignored.rs\n").unwrap();
    std::fs::write(root.0.join("ignored.rs"), "// Rust build input\n").unwrap();
    let after = WorkspaceSnapshot::capture(root.path()).unwrap();
    assert_eq!(
        before.changed_paths(&after),
        [".gitignore", "ignored.rs", "src/lib.rs"]
    );
}

#[test]
fn cargo_config_external_path_override_is_rejected() {
    let root = TestWorkspace::new();
    std::fs::create_dir(root.0.join(".cargo")).unwrap();
    std::fs::write(
        root.0.join(".cargo/config.toml"),
        "paths = [\"../outside\"]\n",
    )
    .unwrap();
    assert!(matches!(
        WorkspaceSnapshot::capture(root.path()),
        Err(VerificationError::Unsupported(_))
    ));
}
