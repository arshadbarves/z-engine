use super::{
    Config, EnvVars, GeneralOverrides, TaskReportView, persist_general, project_config_read_path,
};

#[test]
fn task_report_view_defaults_to_quiet() {
    assert_eq!(Config::default().task_report_view, TaskReportView::Quiet);
}

#[test]
fn task_report_view_parses_all_typed_values() {
    for (value, expected) in [
        ("quiet", TaskReportView::Quiet),
        ("compact", TaskReportView::Compact),
        ("detailed", TaskReportView::Detailed),
    ] {
        let text = format!("task_report_view = \"{value}\"");
        let cfg = Config::layer(
            Some(std::path::Path::new("/tmp/config.toml")),
            Some(&text),
            &EnvVars::default(),
        )
        .unwrap();
        assert_eq!(cfg.task_report_view, expected);
    }
}

#[test]
fn task_report_view_layers_and_rejects_unknown_values() {
    let cfg = Config::layer_all(
        Some(std::path::Path::new("/tmp/global.toml")),
        Some("task_report_view = \"compact\""),
        Some(std::path::Path::new("/tmp/project.toml")),
        Some("task_report_view = \"detailed\""),
        &EnvVars::default(),
    )
    .unwrap();
    assert_eq!(cfg.task_report_view, TaskReportView::Detailed);

    let invalid = Config::layer(
        Some(std::path::Path::new("/tmp/config.toml")),
        Some("task_report_view = \"verbose\""),
        &EnvVars::default(),
    );
    assert!(invalid.is_err());
}

#[test]
fn task_report_view_persists_and_roundtrips() {
    let tmp = tempfile::tempdir().unwrap();
    persist_general(
        tmp.path(),
        &GeneralOverrides {
            task_report_view: Some(TaskReportView::Compact),
            ..GeneralOverrides::default()
        },
    )
    .unwrap();

    let text = std::fs::read_to_string(project_config_read_path(tmp.path())).unwrap();
    assert!(text.contains("task_report_view = \"compact\""));
    let cfg = Config::load(Some(tmp.path())).unwrap();
    assert_eq!(cfg.task_report_view, TaskReportView::Compact);
}
