use super::{persist_project_general_if_valid, project_scoped_general};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use z_engine_core::config::{GeneralOverrides, TaskReportView};

/// Temp project root removed even when an assertion unwinds.
struct Workspace(PathBuf);

impl Workspace {
    fn new(tag: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "z-engine-settings-{tag}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join(".z-engine")).unwrap();
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn project_config(&self) -> PathBuf {
        self.0.join(".z-engine/config.toml")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_general_persistence_propagates_write_errors() {
    let workspace = Workspace::new("persist");
    std::fs::write(workspace.project_config(), "model = ").unwrap();

    let result = persist_project_general_if_valid(
        Some(workspace.path()),
        &GeneralOverrides {
            model: Some("test/model".into()),
            ..GeneralOverrides::default()
        },
    );

    assert!(result.is_err());
}

#[test]
fn project_scope_drops_task_report_view_and_keeps_every_other_setting() {
    let over = GeneralOverrides {
        model: Some("test/model".into()),
        base_url: Some("https://example.test/v1".into()),
        max_context_tokens: Some(4096),
        review_enabled: Some(true),
        max_task_continuations: Some(3),
        task_report_view: Some(TaskReportView::Detailed),
    };

    assert_eq!(
        project_scoped_general(&over),
        GeneralOverrides {
            task_report_view: None,
            ..over
        }
    );
}

#[test]
fn task_report_view_alone_writes_no_project_config() {
    let workspace = Workspace::new("view-only");
    let over = GeneralOverrides {
        task_report_view: Some(TaskReportView::Compact),
        ..GeneralOverrides::default()
    };

    persist_project_general_if_valid(Some(workspace.path()), &project_scoped_general(&over))
        .unwrap();

    assert!(
        !workspace.project_config().exists(),
        "task_report_view is a global UI preference and must not create project config"
    );
}

#[test]
fn project_config_keeps_general_settings_without_task_report_view() {
    let workspace = Workspace::new("mixed");
    let over = GeneralOverrides {
        model: Some("test/model".into()),
        base_url: Some("https://example.test/v1".into()),
        max_context_tokens: Some(4096),
        review_enabled: Some(false),
        max_task_continuations: Some(0),
        task_report_view: Some(TaskReportView::Detailed),
    };

    persist_project_general_if_valid(Some(workspace.path()), &project_scoped_general(&over))
        .unwrap();

    let text = std::fs::read_to_string(workspace.project_config()).unwrap();
    assert!(text.contains("model = \"test/model\""));
    assert!(text.contains("base_url = \"https://example.test/v1\""));
    assert!(text.contains("max_context_tokens = 4096"));
    assert!(text.contains("review = false"));
    assert!(text.contains("max_task_continuations = 0"));
    assert!(
        !text.contains("task_report_view"),
        "project config must not record the global report-view preference: {text}"
    );
}
