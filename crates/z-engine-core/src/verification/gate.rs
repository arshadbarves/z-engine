use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::artifacts::validate_artifact;
use super::{
    CheckEvidence, CheckKind, CheckOutcome, TaskReport, TaskStatus, VerificationError,
    WorkspaceSnapshot,
};

/// Evaluate evidence without persisting or publishing a completion decision.
/// Parent-owned blockers are never cleared by this gate.
pub fn assess(
    report: &mut TaskReport,
    current: &WorkspaceSnapshot,
) -> Result<(), VerificationError> {
    report.status = TaskStatus::NeedsVerification;
    validate_report(report)?;
    let identity = if report.checks.is_empty() {
        String::new()
    } else {
        super::toolchain::identity(&current.root)?
    };
    assess_with_identity(report, current, &identity)
}

pub(super) fn assess_with_identity(
    report: &mut TaskReport,
    current: &WorkspaceSnapshot,
    identity: &str,
) -> Result<(), VerificationError> {
    report.status = TaskStatus::NeedsVerification;
    validate_report(report)?;
    if Path::new(&report.workspace_root)
        .canonicalize()
        .map_err(|e| VerificationError::io(&report.workspace_root, e))?
        != current.root
    {
        report.status = TaskStatus::Stale;
        return Ok(());
    }
    if !report.blockers.is_empty() {
        report.status = TaskStatus::Blocked;
        return Ok(());
    }
    if report.checks.is_empty() {
        return Ok(());
    }
    invalidate_evidence(report, current, identity);
    let pass_index = report.checks.iter().rposition(|check| {
        check.spec.is_full_workspace_test()
            && check.outcome == CheckOutcome::Passed
            && check.tests_run.is_some_and(|count| count > 0)
            && check.exit_code == Some(0)
    });
    let Some(pass_index) = pass_index else {
        report.status = if report
            .checks
            .iter()
            .any(|c| c.outcome == CheckOutcome::Stale)
        {
            TaskStatus::Stale
        } else if report.checks.iter().any(|c| {
            matches!(
                c.outcome,
                CheckOutcome::Failed | CheckOutcome::Blocked | CheckOutcome::Cancelled
            )
        }) {
            TaskStatus::Blocked
        } else {
            TaskStatus::NeedsVerification
        };
        return Ok(());
    };
    if report.checks[pass_index + 1..]
        .iter()
        .any(|check| check.outcome != CheckOutcome::Passed)
    {
        report.status = TaskStatus::Blocked;
        return Ok(());
    }
    let Some(assessment) = &report.assessment else {
        return Ok(());
    };
    if assessment.summary.trim().is_empty()
        || assessment.coverage.len() != report.requirements.len()
    {
        return Ok(());
    }
    let evidence: BTreeMap<_, _> = report.checks.iter().map(|c| (c.id.as_str(), c)).collect();
    let requirements: BTreeSet<_> = report.requirements.iter().map(|r| r.id.as_str()).collect();
    let mut covered = BTreeSet::new();
    for coverage in &assessment.coverage {
        if !requirements.contains(coverage.requirement_id.as_str())
            || !covered.insert(coverage.requirement_id.as_str())
            || coverage.explanation.trim().is_empty()
            || coverage.evidence_ids.is_empty()
        {
            return Ok(());
        }
        for id in &coverage.evidence_ids {
            let Some(check) = evidence.get(id.as_str()) else {
                return Ok(());
            };
            if check.outcome != CheckOutcome::Passed || check.exit_code != Some(0) {
                return Ok(());
            }
        }
    }
    if covered == requirements {
        report.status = TaskStatus::Complete;
    }
    Ok(())
}

/// Revalidate persisted successes, without promoting previously unassessed/running tasks.
pub fn refresh(report: &mut TaskReport) -> Result<(), VerificationError> {
    let previous = report.status;
    if let Err(error) = validate_report(report) {
        if previous == TaskStatus::Complete {
            report.status = TaskStatus::Stale;
        }
        return Err(error);
    }
    if previous != TaskStatus::Complete
        && !report
            .checks
            .iter()
            .any(|check| check.outcome == CheckOutcome::Passed)
    {
        return Ok(());
    }
    let snapshot = match WorkspaceSnapshot::capture(Path::new(&report.workspace_root)) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            for check in &mut report.checks {
                if check.outcome == CheckOutcome::Passed {
                    check.outcome = CheckOutcome::Stale;
                }
            }
            if previous == TaskStatus::Complete {
                report.status = TaskStatus::Stale;
            }
            return Err(error);
        }
    };
    if previous == TaskStatus::Complete {
        if let Err(error) = assess(report, &snapshot) {
            report.status = TaskStatus::Stale;
            return Err(error);
        }
    } else if report
        .checks
        .iter()
        .any(|c| c.outcome == CheckOutcome::Passed)
    {
        let identity = super::toolchain::identity(&snapshot.root)?;
        invalidate_evidence(report, &snapshot, &identity);
    }
    Ok(())
}

fn validate_report(report: &TaskReport) -> Result<(), VerificationError> {
    if report.schema_version != super::TASK_REPORT_SCHEMA_VERSION {
        return Err(VerificationError::InvalidInput(format!(
            "unsupported task schema version {}",
            report.schema_version
        )));
    }
    if report.supervision.as_ref().is_some_and(|state| {
        state.max_continuations > z_engine_runtime::MAX_CONTINUATIONS
            || state.continuations > state.max_continuations
    }) {
        return Err(VerificationError::InvalidInput(
            "task supervision counters exceed their allowed budget".into(),
        ));
    }
    let mut requirements = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    if report.task_id.trim().is_empty()
        || report.goal.trim().is_empty()
        || report.requirements.is_empty()
        || report.requirements.iter().any(|r| {
            r.id.trim().is_empty() || r.description.trim().is_empty() || !requirements.insert(&r.id)
        })
        || report
            .checks
            .iter()
            .any(|c| c.id.trim().is_empty() || !evidence.insert(&c.id))
    {
        return Err(VerificationError::InvalidInput(
            "task, requirements, and evidence must have nonempty unique identities".into(),
        ));
    }
    Ok(())
}

fn invalidate_evidence(report: &mut TaskReport, snapshot: &WorkspaceSnapshot, identity: &str) {
    for check in &mut report.checks {
        if check.outcome == CheckOutcome::Passed {
            if let Err(error) = validate_pass(check, snapshot, identity) {
                check.outcome = CheckOutcome::Stale;
                check.summary = format!("evidence is no longer valid: {error}")
                    .chars()
                    .take(2000)
                    .collect();
            }
        }
    }
}

fn validate_pass(
    check: &CheckEvidence,
    snapshot: &WorkspaceSnapshot,
    identity: &str,
) -> Result<(), VerificationError> {
    let invalid = |message: &str| VerificationError::InvalidInput(message.into());
    if check.input_fingerprint.as_deref() != Some(&snapshot.fingerprint)
        || check.toolchain != identity
        || Path::new(&check.cwd) != snapshot.root
        || check.exit_code != Some(0)
        || check.command != check.spec.command()?
    {
        return Err(invalid(
            "workspace, toolchain, command, or exit status mismatch",
        ));
    }
    let stdout = check
        .stdout
        .as_ref()
        .ok_or_else(|| invalid("missing stdout evidence"))?;
    let stderr = check
        .stderr
        .as_ref()
        .ok_or_else(|| invalid("missing stderr evidence"))?;
    validate_artifact(stdout)?;
    validate_artifact(stderr)?;
    if check.spec.kind == CheckKind::CargoTest {
        let counts = super::runner::test_counts(Path::new(&stdout.path))?;
        if !matches!(counts, Some((count, 0)) if count > 0 && Some(count) == check.tests_run) {
            return Err(invalid(
                "missing, zero, failed, or inconsistent Rust test count",
            ));
        }
    }
    Ok(())
}
