//! The facts a guarded completion is judged against.
//!
//! A plan is assembled by the tools layer at the moment the model claims
//! to be done and handed to [`super::verify::VerificationRunner`]. It
//! carries three independent accounts of the run, and verification is
//! largely the business of reconciling them:
//!
//! - `mutated` — every write the governed tools performed, with the hash
//!   of the bytes they left behind. This is what the harness *authorized*;
//! - `changes` — every difference between the workspace as the run found
//!   it and the workspace as it stands now. This is what *happened*,
//!   including changes no tool of ours made;
//! - `witnesses` — what the run read, and what it hashed at the time.
//!
//! Assembling a plan can fail, and failing is not the same as having
//! nothing to verify: a lost mutation log means the run cannot describe
//! what it did, which has to block rather than read as "changed nothing".

use std::path::PathBuf;

use super::snapshot::{SnapshotError, WorkspaceSnapshot};
use super::work_order::AcceptanceCommand;

/// One path this run read, and the hash it had when it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadWitness {
    /// Repository-relative path, as recorded on the evidence record.
    pub path: PathBuf,
    /// SHA-256 of the whole file at capture time.
    pub file_hash: String,
}

/// One change the harness authorized: a path a governed tool wrote, and
/// the hash of the bytes it wrote there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationRecord {
    /// Repository-relative path.
    pub path: PathBuf,
    /// SHA-256 of the file's contents as the tool left them.
    pub content_hash: String,
}

/// What a path holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeState {
    Present { content_hash: String },
    Missing,
}

/// One difference between the workspace this run started in and the
/// workspace it is asking to complete in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceChange {
    pub path: PathBuf,
    pub state: ChangeState,
}

/// Everything verification needs to decide whether a run may complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPlan {
    pub work_order_id: String,
    pub goal: String,
    /// Repository-relative writable paths the order declared.
    pub scope: Vec<PathBuf>,
    /// Every write the governed tools performed, with its final hash.
    pub mutated: Vec<MutationRecord>,
    /// Every difference the workspace itself shows, authorized or not.
    pub changes: Vec<WorkspaceChange>,
    /// The tree as it stood when this plan was assembled — before any
    /// check ran. Comparing against it afterwards is what separates the
    /// harness's own writes from a change that slipped in behind the
    /// audit.
    pub workspace: WorkspaceSnapshot,
    /// Every path this run read, with the hash it had at read time.
    pub witnesses: Vec<ReadWitness>,
    pub acceptance: Vec<AcceptanceCommand>,
}

impl VerificationPlan {
    /// The paths the governed tools wrote, in log order.
    pub fn mutated_paths(&self) -> Vec<PathBuf> {
        self.mutated.iter().map(|m| m.path.clone()).collect()
    }

    /// The hash the harness last authorized for `path`, if any.
    pub(super) fn authorized_hash(&self, path: &PathBuf) -> Option<&str> {
        self.mutated
            .iter()
            .rev()
            .find(|m| &m.path == path)
            .map(|m| m.content_hash.as_str())
    }
}

/// Why a plan could not be assembled. Every variant blocks completion:
/// each one means the harness cannot say what this run did.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error(
        "this run's record of what it started from and what it changed is unreadable, so \
         completion cannot be verified; start a new run"
    )]
    TurnRecordUnavailable,
    #[error(
        "this run's record of what it read is unreadable, so completion cannot be verified; start \
         a new run"
    )]
    WitnessesUnavailable,
    #[error(
        "this run changed files with no active work order, so there is nothing to verify against"
    )]
    NoActiveOrder,
    #[error("{0}")]
    Workspace(#[from] SnapshotError),
}
