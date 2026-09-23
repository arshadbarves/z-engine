use serde::{Deserialize, Serialize};

use crate::{CheckOutcome, EvidenceArtifact, ModelNote, Requirement, TaskStatus};

pub const CONTEXT_PACKET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPacket {
    #[serde(default)]
    pub kind: PacketKind,
    pub schema_version: u32,
    pub provenance: PacketProvenance,
    pub harness: HarnessObservation,
    pub evidence_details: Vec<EvidenceDetails>,
    pub changed_paths: Vec<String>,
    /// Replacement summaries are retained even over budget, but remain unverified.
    pub model_notes: Vec<ModelNote>,
    pub omitted: OmittedCounts,
    pub budget: BudgetUsage,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketKind {
    #[default]
    TaskContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketProvenance {
    pub source: ObservationSource,
    pub report_schema_version: u32,
    pub task_id: String,
    pub workspace_root: String,
    pub freshness: Freshness,
    /// TaskReport currently has no report-level observation timestamp.
    pub report_observed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    HarnessTaskReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    ReportSnapshotNotRevalidated,
}

/// These fields are protected even when they exceed the requested byte budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessObservation {
    pub original_goal: String,
    pub active_requirements: Vec<Requirement>,
    pub observed_status: TaskStatus,
    pub blockers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervision: Option<serde_json::Value>,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub evidence_id: String,
    pub observed_outcome: CheckOutcome,
    /// Source version recorded by verification; never calculated by this builder.
    pub input_fingerprint: Option<String>,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub tests_run: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDetails {
    pub evidence_id: String,
    pub command: Vec<String>,
    pub cwd: String,
    pub toolchain: String,
    pub summary: String,
    pub stdout: Option<EvidenceArtifact>,
    pub stderr: Option<EvidenceArtifact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmittedCounts {
    pub evidence_details: usize,
    pub changed_paths: usize,
    pub model_notes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetUsage {
    /// Target for the entire compact JSON packet, in UTF-8 bytes (not tokens).
    pub target_bytes: usize,
    pub serialized_bytes: usize,
    /// Nonzero only when the protected packet itself exceeds the target.
    pub over_budget_bytes: usize,
}
