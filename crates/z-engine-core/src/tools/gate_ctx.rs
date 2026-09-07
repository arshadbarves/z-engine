//! The tools-layer adapter for the governance mutation gate: applies the
//! pure gate's verdict to facts the rest of the run already owns.
//!
//! Thin by design, and staged by cost. Canonical path identity and
//! evidence freshness come from [`ToolCtx`] (Task 3) and the active order
//! from the work-order store (Task 4); those settle
//! [`GateEngine::prescreen`] with no I/O. Only a request that survives it
//! pays for Rust semantic facts (see [`super::gate_facts`]), so a missing
//! work order never waits on rust-analyzer. Nothing here re-implements
//! hashing, path normalization, or symbol discovery, and none of the
//! *rules* live here: they live in the pure gate.

use std::path::Path;

use crate::evidence::{BlobHandle, EvidenceRecord};
use crate::governance::{
    EvidenceState, EvidenceView, GateEngine, GateFailure, LineRange, MutationRequest,
};
use crate::perms::PolicyEngine;

use super::ToolCtx;
use super::gate_facts::is_rust;
use super::path_identity::canonical_in_root;

impl ToolCtx {
    /// Authorize one mutation before it reaches disk.
    ///
    /// `current` must be the exact bytes the caller is about to replace —
    /// the snapshot it already read — so freshness is judged against what
    /// is being overwritten rather than a second, racy read. `changed` is
    /// the 1-based inclusive span those bytes lose or gain (`None` for a
    /// whole-file write or a creation); [`crate::governance::changed_line_range`]
    /// computes it.
    ///
    /// Unguarded runs (no work-order store attached) authorize everything,
    /// exactly as before governance existed.
    pub async fn authorize_mutation(
        &self,
        path: &Path,
        current: &[u8],
        changed: Option<LineRange>,
    ) -> Result<(), GateFailure> {
        if self.work_orders.is_none() {
            return Ok(());
        }
        let order = self.active_work_order();
        let identity = self.repo_relative_identity(path);
        let target = identity
            .clone()
            .unwrap_or_else(|| path.display().to_string());
        let request = MutationRequest {
            path,
            identity: identity.as_deref(),
            order: order.as_deref(),
            changed,
            evidence: self.evidence_state(path, current),
            rust: is_rust(path),
        };
        let prescreen = GateEngine::prescreen(&request);
        let verdict = if !prescreen.is_pass() || !request.rust {
            prescreen.into_result()
        } else {
            // Semantics are gathered only for a change that is otherwise
            // authorized, and are the only thing that can localize it.
            let facts = self.rust_facts(path, current).await;
            GateEngine::authorize(&request, Some(&facts)).into_result()
        };
        self.record_gate_decision(crate::replay::GateKind::Mutation, &target, &verdict);
        verdict
    }

    /// Authorize one shell command. Guarded runs only run commands whose
    /// write set is provably empty; session prefix rules deliberately do
    /// not count, since they authorize *approval*, not proof.
    pub fn authorize_command(&self, command: &str) -> Result<(), GateFailure> {
        if self.work_orders.is_none() {
            return Ok(());
        }
        let verdict =
            GateEngine::authorize_command(command, PolicyEngine::is_provably_read_only(command))
                .into_result();
        self.record_gate_decision(crate::replay::GateKind::Command, command, &verdict);
        verdict
    }

    /// Compare the run's latest read of `path` against the bytes about to
    /// change, reusing the evidence module's content hash.
    fn evidence_state(&self, path: &Path, current: &[u8]) -> EvidenceState {
        let Some(record) = self.latest_read_evidence(path) else {
            return EvidenceState::Missing;
        };
        if BlobHandle::of(current).to_string() == record.file_hash {
            EvidenceState::Fresh {
                id: record.id,
                covered: record.line_range,
            }
        } else {
            EvidenceState::Stale
        }
    }

    /// The most recent record captured for `path`, fresh or not — the
    /// gate needs the distinction that [`ToolCtx::fresh_read_evidence`]
    /// deliberately collapses, so it can say *why* it refused.
    fn latest_read_evidence(&self, path: &Path) -> Option<EvidenceRecord> {
        let canonical = canonical_in_root(&self.resolve(path), &self.project_root)?;
        self.evidence.as_ref()?.latest_for(&canonical)
    }
}

#[cfg(test)]
mod tests;
