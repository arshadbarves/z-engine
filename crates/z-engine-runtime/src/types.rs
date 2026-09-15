use serde::{Deserialize, Serialize};

pub const MAX_CONTINUATIONS: u32 = 10;

#[derive(Debug, thiserror::Error)]
pub enum SupervisionError {
    #[error("task continuation limit {0} exceeds the maximum of 10")]
    InvalidLimit(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkAction {
    Continue,
    Verify,
    Repair,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Boundary {
    Complete,
    Cancelled,
    Ineligible,
    Blocked(String),
    Incomplete {
        /// A fingerprint of observed inputs/results, excluding generated IDs,
        /// elapsed time, and model claims, which cannot demonstrate progress.
        progress_key: String,
        next: WorkAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionAction {
    Continue,
    Verify,
    Repair,
    Complete,
    Blocked,
    Idle,
    Stopped,
    Interrupted,
}

impl SupervisionAction {
    pub fn continues(self) -> bool {
        matches!(self, Self::Continue | Self::Verify | Self::Repair)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionReport {
    pub continuations: u32,
    pub max_continuations: u32,
    pub last_action: SupervisionAction,
    pub reason: String,
}
