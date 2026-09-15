use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const TASK_REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("verification I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid verification input: {0}")]
    InvalidInput(String),
    #[error("unsupported verification scope: {0}")]
    Unsupported(String),
    #[error("verification scan limit exceeded: {0}")]
    ScanLimit(String),
    #[error("verification process failed: {0}")]
    Process(String),
    #[error("verification worker failed: {0}")]
    Worker(#[from] tokio::task::JoinError),
}

impl VerificationError {
    pub(super) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskReport {
    pub schema_version: u32,
    pub task_id: String,
    pub goal: String,
    pub workspace_root: String,
    pub status: TaskStatus,
    pub requirements: Vec<Requirement>,
    pub checks: Vec<CheckEvidence>,
    pub assessment: Option<CompletionAssessment>,
    pub blockers: Vec<String>,
    pub changed_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervision: Option<z_engine_runtime::SupervisionReport>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletionAssessment {
    pub summary: String,
    pub coverage: Vec<RequirementCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementCoverage {
    pub requirement_id: String,
    pub evidence_ids: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckSpec {
    pub kind: CheckKind,
    pub package: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    CargoTest,
    CargoBuild,
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
pub struct CheckEvidence {
    pub id: String,
    pub spec: CheckSpec,
    pub command: Vec<String>,
    pub cwd: String,
    pub input_fingerprint: Option<String>,
    pub toolchain: String,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub tests_run: Option<u64>,
    pub outcome: CheckOutcome,
    pub summary: String,
    pub stdout: Option<EvidenceArtifact>,
    pub stderr: Option<EvidenceArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceArtifact {
    pub path: String,
    pub digest: String,
}
