use std::collections::BTreeSet;

use crate::{
    BudgetUsage, CONTEXT_PACKET_SCHEMA_VERSION, CheckOutcome, ContextError, ContextPacket,
    EvidenceDetails, EvidenceRef, Freshness, HarnessObservation, ModelNote, NoteKind,
    ObservationSource, OmittedCounts, PacketKind, PacketProvenance, TaskObservation,
};

/// Protect task state, evidence identities and replacement summaries.
pub fn build_packet(
    task: TaskObservation,
    notes: Vec<ModelNote>,
    target_bytes: usize,
) -> Result<ContextPacket, ContextError> {
    validate(&task)?;
    // These summaries replaced prose in working history, so evicting them would
    // remove both representations. Retention does not elevate their trust.
    let (summaries, notes): (Vec<_>, Vec<_>) = notes
        .into_iter()
        .partition(|note| note.kind == NoteKind::Summary);
    let mut packet = ContextPacket {
        kind: PacketKind::TaskContext,
        schema_version: CONTEXT_PACKET_SCHEMA_VERSION,
        provenance: PacketProvenance {
            source: ObservationSource::HarnessTaskReport,
            report_schema_version: task.report_schema_version,
            task_id: task.task_id,
            workspace_root: task.workspace_root,
            freshness: Freshness::ReportSnapshotNotRevalidated,
            report_observed_at_ms: None,
        },
        harness: HarnessObservation {
            original_goal: task.goal,
            active_requirements: task.requirements,
            observed_status: task.status,
            blockers: task.blockers,
            supervision: task.supervision,
            evidence_refs: task
                .checks
                .iter()
                .map(|check| EvidenceRef {
                    evidence_id: check.id.clone(),
                    observed_outcome: check.outcome,
                    input_fingerprint: check.input_fingerprint.clone(),
                    started_at_ms: check.started_at_ms,
                    duration_ms: check.duration_ms,
                    exit_code: check.exit_code,
                    tests_run: check.tests_run,
                })
                .collect(),
        },
        evidence_details: Vec::new(),
        changed_paths: Vec::new(),
        model_notes: summaries,
        omitted: OmittedCounts {
            evidence_details: task.checks.len(),
            changed_paths: task.changed_paths.len(),
            model_notes: notes.len(),
        },
        budget: BudgetUsage {
            target_bytes,
            serialized_bytes: 0,
            over_budget_bytes: 0,
        },
    };
    update_usage(&mut packet)?;
    if packet.budget.over_budget_bytes > 0 {
        return Ok(packet);
    }

    // Non-passing checks precede passing checks; newest report entries win ties.
    let mut checks: Vec<_> = task.checks.into_iter().rev().collect();
    checks.sort_by_key(|check| check.outcome == CheckOutcome::Passed);
    for check in checks {
        packet.evidence_details.push(EvidenceDetails {
            evidence_id: check.id,
            command: check.command,
            cwd: check.cwd,
            toolchain: check.toolchain,
            summary: check.summary,
            stdout: check.stdout,
            stderr: check.stderr,
        });
        packet.omitted.evidence_details -= 1;
        update_usage(&mut packet)?;
        if packet.budget.over_budget_bytes > 0 {
            packet.evidence_details.pop();
            packet.omitted.evidence_details += 1;
            update_usage(&mut packet)?;
        }
    }
    for path in task.changed_paths {
        packet.changed_paths.push(path);
        packet.omitted.changed_paths -= 1;
        update_usage(&mut packet)?;
        if packet.budget.over_budget_bytes > 0 {
            packet.changed_paths.pop();
            packet.omitted.changed_paths += 1;
            update_usage(&mut packet)?;
        }
    }
    for note in notes {
        packet.model_notes.push(note);
        packet.omitted.model_notes -= 1;
        update_usage(&mut packet)?;
        if packet.budget.over_budget_bytes > 0 {
            packet.model_notes.pop();
            packet.omitted.model_notes += 1;
            update_usage(&mut packet)?;
        }
    }
    Ok(packet)
}

impl ContextPacket {
    pub fn to_json(&self) -> Result<String, ContextError> {
        Ok(serde_json::to_string(self)?)
    }
}

fn update_usage(packet: &mut ContextPacket) -> Result<(), ContextError> {
    // Start at zero for a canonical fixed point, including decimal digit boundaries.
    packet.budget.serialized_bytes = 0;
    packet.budget.over_budget_bytes = 0;
    loop {
        let bytes = serde_json::to_vec(packet)?.len();
        let overflow = bytes.saturating_sub(packet.budget.target_bytes);
        if bytes == packet.budget.serialized_bytes && overflow == packet.budget.over_budget_bytes {
            return Ok(());
        }
        packet.budget.serialized_bytes = bytes;
        packet.budget.over_budget_bytes = overflow;
    }
}

fn validate(task: &TaskObservation) -> Result<(), ContextError> {
    for (field, value) in [
        ("task id", task.task_id.as_str()),
        ("original goal", task.goal.as_str()),
        ("workspace root", task.workspace_root.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ContextError::EmptyField(field));
        }
    }
    if task.requirements.is_empty() {
        return Err(ContextError::EmptyField("active requirements"));
    }
    let mut ids = BTreeSet::new();
    for requirement in &task.requirements {
        if requirement.id.trim().is_empty() || requirement.description.trim().is_empty() {
            return Err(ContextError::EmptyField("requirement id and description"));
        }
        if !ids.insert(&requirement.id) {
            return Err(ContextError::DuplicateId {
                kind: "requirement",
                id: requirement.id.clone(),
            });
        }
    }
    ids.clear();
    for check in &task.checks {
        if check.id.trim().is_empty() {
            return Err(ContextError::EmptyField("evidence id"));
        }
        if !ids.insert(&check.id) {
            return Err(ContextError::DuplicateId {
                kind: "evidence",
                id: check.id.clone(),
            });
        }
    }
    Ok(())
}
