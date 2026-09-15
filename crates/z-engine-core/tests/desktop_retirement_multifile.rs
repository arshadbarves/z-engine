//! Preserve multi-file feature acceptance without a live model or terminal frontend.

#[path = "support/desktop_retirement_fixture.rs"]
mod fixture;

use fixture::{Provider, answer, config, next_event, shutdown, tool};
use serde_json::json;
use z_engine_core::agent::{ApprovalDecision, Event, spawn_with_recorder};
use z_engine_core::session::{SessionEvent, SessionWriter, read_events};
use z_engine_core::verification::CheckOutcome;

const TASKS_BEFORE: &str = r#"pub struct Task {
    title: String,
}
impl Task {
    pub fn new(title: &str) -> Self { Self { title: title.into() } }
    pub fn display(&self) -> String { self.title.clone() }
}
#[cfg(test)]
mod tests {
    use super::Task;
    #[test]
    fn keeps_title() { assert_eq!(Task::new("write docs").title, "write docs"); }
}
"#;

const TASKS_AFTER: &str = r#"pub struct Task {
    title: String,
    priority: u8,
}
impl Task {
    pub fn new(title: &str) -> Self { Self { title: title.into(), priority: 1 } }
    pub fn priority(&self) -> u8 { self.priority }
    pub fn set_priority(&mut self, priority: u8) { self.priority = priority; }
    pub fn display(&self) -> String { format!("[P{}] {}", self.priority, self.title) }
}
#[cfg(test)]
mod tests {
    use super::Task;
    #[test]
    fn keeps_title() { assert_eq!(Task::new("write docs").title, "write docs"); }
    #[test]
    fn default_priority() { assert_eq!(Task::new("write docs").priority(), 1); }
    #[test]
    fn changes_priority() {
        let mut task = Task::new("write docs");
        task.set_priority(3);
        assert_eq!(task.priority(), 3);
    }
    #[test]
    fn displays_priority() {
        let mut task = Task::new("write docs");
        task.set_priority(3);
        assert_eq!(task.display(), "[P3] write docs");
    }
}
"#;

const LIB_BEFORE: &str = r#"mod tasks;
pub use tasks::Task;
pub fn render_task(title: &str) -> String { Task::new(title).display() }
"#;

const LIB_AFTER: &str = r#"mod tasks;
pub use tasks::Task;
pub fn render_task(title: &str, priority: Option<u8>) -> String {
    let mut task = Task::new(title);
    if let Some(priority) = priority { task.set_priority(priority); }
    task.display()
}
#[cfg(test)]
mod tests {
    #[test]
    fn renders_default() { assert_eq!(super::render_task("write docs", None), "[P1] write docs"); }
    #[test]
    fn renders_requested_priority() {
        assert_eq!(super::render_task("write docs", Some(3)), "[P3] write docs");
    }
}
"#;

#[tokio::test]
async fn implements_and_verifies_priority_across_files_after_approval() {
    let directory = tempfile::Builder::new()
        .prefix(".desktop-multifile-")
        .tempdir_in(".")
        .unwrap();
    let root = directory.path().canonicalize().unwrap();
    let workspace = root.join("workspace");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::create_dir_all(workspace.join(".cargo")).unwrap();
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"priority-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(
        workspace.join(".cargo/config.toml"),
        "[net]\noffline = true\n",
    )
    .unwrap();
    std::fs::write(workspace.join("src/tasks.rs"), TASKS_BEFORE).unwrap();
    std::fs::write(workspace.join("src/lib.rs"), LIB_BEFORE).unwrap();
    let lockfile = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::process::Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&workspace)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("fixture lockfile generation timed out")
    .unwrap();
    assert!(lockfile.status.success(), "{lockfile:?}");
    let provider = Provider::start(vec![
        tool(
            "baseline",
            "run_verification",
            json!({"kind": "cargo_test"}),
        ),
        tool("read-tasks", "read_file", json!({"path": "src/tasks.rs"})),
        tool("read-lib", "read_file", json!({"path": "src/lib.rs"})),
        tool(
            "edit-tasks",
            "edit_file",
            json!({"path": "src/tasks.rs", "old_string": TASKS_BEFORE, "new_string": TASKS_AFTER}),
        ),
        tool(
            "edit-lib",
            "edit_file",
            json!({"path": "src/lib.rs", "old_string": LIB_BEFORE, "new_string": LIB_AFTER}),
        ),
        tool("feature", "run_verification", json!({"kind": "cargo_test"})),
        answer("Priority is wired through the task and its caller."),
    ])
    .await;
    let session = root.join("feature.jsonl");
    let recorder = SessionWriter::append_to(&session).unwrap();
    let (handle, mut events) =
        spawn_with_recorder(config(&provider, &workspace), None, Some(recorder));
    handle.submit("Add default priority 1, a getter and setter, and priority display to Task. Wire optional priority through render_task and verify both files.");
    let mut approvals = Vec::new();
    let mut finished_tools = 0;
    loop {
        match next_event(&mut events).await {
            Event::ApprovalRequired {
                id,
                tool,
                detail_preview,
                ..
            } => {
                if tool == "edit_file" {
                    assert!(
                        detail_preview
                            .as_ref()
                            .is_some_and(|diff| diff.contains("priority"))
                    );
                    let (path, before) = if approvals.iter().any(|tool| tool == "edit_file") {
                        ("src/lib.rs", LIB_BEFORE)
                    } else {
                        ("src/tasks.rs", TASKS_BEFORE)
                    };
                    assert_eq!(
                        std::fs::read_to_string(workspace.join(path)).unwrap(),
                        before
                    );
                } else {
                    assert_eq!(tool, "run_verification");
                }
                approvals.push(tool);
                handle.approve(id, ApprovalDecision::Once);
            }
            Event::ToolCallFinished { ok, summary, .. } => {
                assert!(ok, "{summary}");
                finished_tools += 1;
            }
            Event::TurnCompleted { .. } => break,
            _ => {}
        }
    }
    shutdown(handle, events).await;
    provider.assert_consumed();
    assert_eq!(finished_tools, 6);
    assert_eq!(
        approvals,
        [
            "run_verification",
            "edit_file",
            "edit_file",
            "run_verification"
        ]
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/tasks.rs")).unwrap(),
        TASKS_AFTER
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        LIB_AFTER
    );

    let persisted = read_events(&session).unwrap();
    let baseline = persisted
        .iter()
        .find_map(|event| match event {
            SessionEvent::TaskUpdated { report } => report.checks.first(),
            _ => None,
        })
        .unwrap();
    assert_eq!(baseline.outcome, CheckOutcome::Passed);
    assert_eq!(baseline.tests_run, Some(1));
    let report = persisted
        .iter()
        .rev()
        .find_map(|event| match event {
            SessionEvent::TaskUpdated { report } => Some(report),
            _ => None,
        })
        .unwrap();
    assert_eq!(report.checks.len(), 2);
    assert_eq!(report.checks[0].outcome, CheckOutcome::Stale);
    assert_eq!(report.checks[0].tests_run, Some(1));
    assert_eq!(report.checks[1].outcome, CheckOutcome::Passed);
    assert_eq!(report.checks[1].tests_run, Some(6));
    assert_eq!(report.changed_paths, ["src/lib.rs", "src/tasks.rs"]);
    let requests = provider.requests();
    let last = requests.last().unwrap();
    assert!(
        last["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| { message["role"] == "tool" && message["tool_call_id"] == "feature" })
    );
}
