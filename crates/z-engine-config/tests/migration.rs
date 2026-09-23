//! v1 files are migrated in place with a backup: the user config by
//! `Paths::ensure`, project files on load.

#[allow(dead_code)]
mod support;

use support::{fixture, read, write};
use z_engine_config::{MigrationOutcome, migrate_file, project_config_file, project_local_file};

/// The v1 default user config after a few edits from the v1 settings screen.
const V1_USER: &str = r#"# z-engine configuration
# OpenRouter API key lives in auth.json next to this file (Settings → General).
model = "openrouter/free"
base_url = "https://openrouter.ai/api/v1/"
# Bounded task continuations (0 disables; maximum 10). New sessions only.
max_task_continuations = 3
max_context_tokens = 64000
review = true
shell_path = "C:\\Program Files\\Git\\bin\\bash.exe"

[hooks]
session_start = "echo hello"
turn_completed = "cargo fmt"

[permissions]
allow = ["cargo test*", "git status"]

[mcp.servers.fs]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem"]

[cost.overrides]
"my/model" = { usd_per_mtok_input = 1.0, usd_per_mtok_output = 2.0 }
"#;

/// What v1's project writer produced.
const V1_PROJECT: &str = r#"# z-engine project configuration
# bash prefix rules under [permissions.allow] skip approval for this project.
model = "z/a"
compact_at_percent = 85
task_report_view = "compact"

[permissions]
allow = ["npm run*"]
"#;

#[test]
fn ensure_migrates_the_v1_user_config_with_a_backup() {
    let f = fixture();
    write(&f.paths.user_config_file, V1_USER);
    let outcome = f.paths.ensure().unwrap();
    assert!(outcome.migrated);
    let backup = outcome.backup.clone().unwrap();
    assert_eq!(backup, f.paths.config_dir.join("config.v1.toml"));
    assert_eq!(read(&backup), V1_USER);
    assert!(
        outcome.notes.iter().any(|note| note.contains("`review`")),
        "{:?}",
        outcome.notes
    );
    assert!(read(&f.paths.user_config_file).contains("schema = 2"));

    let loaded = f.load();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let s = &loaded.settings;
    assert_eq!(s.model.main, "openrouter/free");
    assert_eq!(s.model.context_window, Some(64_000));
    assert_eq!(s.provider.base_url, "https://openrouter.ai/api/v1");
    assert_eq!(s.verification.max_continuations, 3);
    assert_eq!(
        s.shell.path.as_deref(),
        Some("C:\\Program Files\\Git\\bin\\bash.exe")
    );
    assert_eq!(s.hooks["SessionStart"][0].command, "echo hello");
    assert_eq!(s.hooks["Stop"][0].command, "cargo fmt");
    assert_eq!(
        s.permissions.allow,
        ["Bash(cargo test:*)", "Bash(git status)"]
    );
    assert_eq!(s.mcp.servers["fs"].command.as_deref(), Some("npx"));
    assert_eq!(s.mcp.servers["fs"].args.len(), 2);
    let pricing = s.pricing["my/model"];
    assert_eq!((pricing.input, pricing.output), (1.0, 2.0));

    assert_eq!(f.paths.ensure().unwrap(), MigrationOutcome::default());
}

#[test]
fn project_files_migrate_on_load_once() {
    let f = fixture();
    let project = project_config_file(&f.project);
    let local = project_local_file(&f.project);
    write(&project, V1_PROJECT);
    write(&local, "max_output_tokens = 4096\n");
    let loaded = f.load();
    let s = &loaded.settings;
    assert_eq!(s.model.main, "z/a");
    assert_eq!(s.model.max_output_tokens, 4096);
    assert_eq!(s.context.compact_at_percent, 85);
    assert_eq!(s.permissions.allow, ["Bash(npm run:*)"]);
    let migrated = loaded
        .warnings
        .iter()
        .filter(|w| w.contains("migrated from v1"))
        .count();
    assert_eq!(migrated, 2, "{:?}", loaded.warnings);
    assert_eq!(
        read(&f.project.join(".z-engine/config.v1.toml")),
        V1_PROJECT
    );
    assert!(f.project.join(".z-engine/config.local.v1.toml").is_file());

    let again = f.load();
    assert_eq!(again.settings, loaded.settings);
    assert!(again.warnings.is_empty(), "{:?}", again.warnings);
}

#[test]
fn an_existing_backup_is_never_overwritten() {
    let f = fixture();
    let project = project_config_file(&f.project);
    let first_backup = f.project.join(".z-engine/config.v1.toml");
    write(&first_backup, "model = \"older\"\n");
    write(&project, V1_PROJECT);
    let outcome = migrate_file(&project).unwrap();
    assert_eq!(
        outcome.backup,
        Some(f.project.join(".z-engine/config.v1.1.toml"))
    );
    assert_eq!(read(&first_backup), "model = \"older\"\n");
    assert_eq!(
        read(&f.project.join(".z-engine/config.v1.1.toml")),
        V1_PROJECT
    );
}

#[test]
fn unparseable_files_are_left_untouched() {
    let f = fixture();
    write(&f.paths.user_config_file, "model = \n");
    let outcome = f.paths.ensure().unwrap();
    assert!(!outcome.migrated && !outcome.notes.is_empty());
    assert_eq!(read(&f.paths.user_config_file), "model = \n");
    assert!(!f.paths.config_dir.join("config.v1.toml").exists());
    let loaded = f.load();
    assert!(loaded.layers[1].error.is_some());
}

#[test]
fn current_and_missing_files_are_not_migrated() {
    let f = fixture();
    let project = project_config_file(&f.project);
    assert_eq!(migrate_file(&project).unwrap(), MigrationOutcome::default());
    write(&project, "schema = 2\n[model]\nmain = \"m\"\n");
    assert_eq!(migrate_file(&project).unwrap(), MigrationOutcome::default());
    write(&project, "");
    assert_eq!(migrate_file(&project).unwrap(), MigrationOutcome::default());
    assert_eq!(read(&project), "");
}

#[cfg(unix)]
#[test]
fn a_read_only_v1_file_is_converted_in_memory() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let project = project_config_file(&f.project);
    write(&project, V1_PROJECT);
    let dir = project.parent().unwrap();
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = std::fs::write(dir.join("probe"), "").is_ok();
    let loaded = f.load();
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        return;
    }
    assert_eq!(loaded.settings.model.main, "z/a");
    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.contains("could not be saved")),
        "{:?}",
        loaded.warnings
    );
    assert_eq!(read(&project), V1_PROJECT);
}
