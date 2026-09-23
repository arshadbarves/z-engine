use serde::{Deserialize, Serialize};

/// An in-memory projection of the harness report, not a new workspace observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskObservation {
    pub report_schema_version: u32,
    pub task_id: String,
    pub goal: String,
    pub workspace_root: String,
    pub status: TaskStatus,
    pub requirements: Vec<Requirement>,
    pub blockers: Vec<String>,
    /// Runtime-owned report JSON, projected without coupling to a runtime crate.
    pub supervision: Option<serde_json::Value>,
    pub checks: Vec<CheckObservation>,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    NeedsVerification,
    Complete,
    Blocked,
    Stopped,
    Interrupted,
    Unassessed,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckObservation {
    pub id: String,
    pub outcome: CheckOutcome,
    pub input_fingerprint: Option<String>,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub tests_run: Option<u64>,
    pub command: Vec<String>,
    pub cwd: String,
    pub toolchain: String,
    pub summary: String,
    pub stdout: Option<EvidenceArtifact>,
    pub stderr: Option<EvidenceArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Passed,
    Failed,
    Blocked,
    Cancelled,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceArtifact {
    pub path: String,
    pub digest: String,
}
