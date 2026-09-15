use super::*;
use std::path::Path;

#[test]
fn supervision_defaults_and_bounds() {
    assert_eq!(Config::default().max_task_continuations, 3);
    for limit in 0..=MAX_TASK_CONTINUATIONS {
        let text = format!("max_task_continuations = {limit}");
        let cfg = Config::layer(
            Some(Path::new("/tmp/config.toml")),
            Some(&text),
            &EnvVars::default(),
        )
        .unwrap();
        assert_eq!(cfg.max_task_continuations, limit);
    }
}

#[test]
fn supervision_rejects_invalid_config_values() {
    for value in ["-1", "1.5", "\"3\"", "4294967296"] {
        let text = format!("max_task_continuations = {value}");
        let result = Config::layer(
            Some(Path::new("/tmp/config.toml")),
            Some(&text),
            &EnvVars::default(),
        );
        assert!(matches!(result, Err(ConfigError::Parse { .. })), "{value}");
    }
    for value in [11, u32::MAX] {
        let text = format!("max_task_continuations = {value}");
        let result = Config::layer(
            Some(Path::new("/tmp/config.toml")),
            Some(&text),
            &EnvVars::default(),
        );
        assert!(matches!(
            result,
            Err(ConfigError::InvalidTaskContinuations(actual)) if actual == value
        ));
    }
}

#[test]
fn project_supervision_overrides_global_without_changing_environment_precedence() {
    let cfg = Config::layer_all(
        Some(Path::new("/tmp/global.toml")),
        Some("max_task_continuations = 8\nmodel = \"global\""),
        Some(Path::new("/tmp/project.toml")),
        Some("max_task_continuations = 0\nmodel = \"project\""),
        &EnvVars {
            harness_model: Some("environment".into()),
            ..EnvVars::default()
        },
    )
    .unwrap();
    assert_eq!(cfg.max_task_continuations, 0);
    assert_eq!(cfg.model, "environment");
}

#[test]
fn supervision_persists_and_survives_unrelated_updates() {
    let tmp = tempfile::tempdir().unwrap();
    for value in [0, 3, MAX_TASK_CONTINUATIONS] {
        persist_general(
            tmp.path(),
            &GeneralOverrides {
                max_task_continuations: Some(value),
                ..GeneralOverrides::default()
            },
        )
        .unwrap();
        persist_bash_rule(tmp.path(), "cargo test*").unwrap();
        persist_general(
            tmp.path(),
            &GeneralOverrides {
                review_enabled: Some(false),
                ..GeneralOverrides::default()
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(project_config_path(tmp.path())).unwrap();
        assert!(text.contains(&format!("max_task_continuations = {value}")));
        assert_eq!(
            Config::load(Some(tmp.path()))
                .unwrap()
                .max_task_continuations,
            value
        );
    }
}

#[test]
fn invalid_supervision_never_overwrites_the_config() {
    let tmp = tempfile::tempdir().unwrap();
    persist_bash_rule(tmp.path(), "cargo test*").unwrap();
    let path = project_config_path(tmp.path());
    let before = std::fs::read(&path).unwrap();
    let error = persist_general(
        tmp.path(),
        &GeneralOverrides {
            max_task_continuations: Some(11),
            ..GeneralOverrides::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(std::fs::read(path).unwrap(), before);
}
