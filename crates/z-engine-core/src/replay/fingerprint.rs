//! Verdict fingerprints: the parts of a verification two runs of the
//! same work must agree on, reduced to a hash.
//!
//! A [`VerificationManifest`] carries durations and captured output —
//! the same verdict looks different every time it is reached. What must
//! not differ is what it *decided*: the order and goal it verified, the
//! scope it declared, what it found changed, what it found out of scope,
//! and how each check ended. The manifest fingerprint is exactly that,
//! and nothing else.
//!
//! The diff fingerprint is the run's change set — every path the
//! workspace shows a difference at, with the content it ended holding.
//! A run that changed nothing has no diff, which is `None` rather than
//! the hash of an empty string: "no change" and "changed to nothing"
//! are different claims.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::governance::{ChangeState, VerificationManifest, VerificationPlan};

use super::entry::content_hash;

/// Hash of what a verification decided.
pub fn manifest_fingerprint(manifest: &VerificationManifest) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "order {}", manifest.work_order_id);
    let _ = writeln!(out, "goal {}", manifest.goal);
    for path in sorted_display(manifest.scope.iter()) {
        let _ = writeln!(out, "scope {path}");
    }
    for path in sorted_display(manifest.mutated.iter()) {
        let _ = writeln!(out, "mutated {path}");
    }
    let mut breaches: Vec<String> = manifest
        .breaches
        .iter()
        .map(|b| format!("breach {} {}", b.path.display(), b.reason))
        .collect();
    breaches.sort();
    for breach in breaches {
        let _ = writeln!(out, "{breach}");
    }
    for check in &manifest.checks {
        // Status is serialized rather than described: a new variant then
        // changes the fingerprint instead of quietly hashing the same.
        let status =
            serde_json::to_string(&check.status).unwrap_or_else(|_| format!("{:?}", check.status));
        let _ = writeln!(
            out,
            "check {} {} required={} {status}",
            check.name, check.command, check.required
        );
    }
    content_hash(out.as_bytes())
}

/// Hash of the workspace difference a run is asking to complete with,
/// or `None` when it changed nothing.
pub fn diff_fingerprint(plan: &VerificationPlan) -> Option<String> {
    if plan.changes.is_empty() {
        return None;
    }
    let entries: BTreeMap<String, String> = plan
        .changes
        .iter()
        .map(|change| {
            let state = match &change.state {
                ChangeState::Present { content_hash } => format!("present {content_hash}"),
                ChangeState::Missing => "missing".to_string(),
            };
            (change.path.display().to_string(), state)
        })
        .collect();
    let mut out = String::new();
    for (path, state) in entries {
        let _ = writeln!(out, "{path} {state}");
    }
    Some(content_hash(out.as_bytes()))
}

fn sorted_display<'a>(paths: impl Iterator<Item = &'a std::path::PathBuf>) -> Vec<String> {
    let mut out: Vec<String> = paths.map(|p| p.display().to_string()).collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{CheckOutcome, CheckStatus, ScopeBreach, WorkspaceChange};
    use std::path::PathBuf;

    fn manifest() -> VerificationManifest {
        VerificationManifest {
            work_order_id: "wo-1".into(),
            goal: "describe the fixture".into(),
            scope: vec![PathBuf::from("Cargo.toml")],
            mutated: vec![PathBuf::from("Cargo.toml")],
            breaches: vec![],
            checks: vec![CheckOutcome {
                name: "cargo-check".into(),
                command: "cargo check".into(),
                required: true,
                status: CheckStatus::Passed,
                duration_ms: 1_234,
                output_tail: "Finished in 1.2s".into(),
            }],
        }
    }

    /// The same verdict reached on a slower machine is the same verdict.
    #[test]
    fn duration_and_output_do_not_change_the_verdict_fingerprint() {
        let baseline = manifest_fingerprint(&manifest());
        let mut slower = manifest();
        slower.checks[0].duration_ms = 90_000;
        slower.checks[0].output_tail = "Finished in 90.0s".into();
        assert_eq!(manifest_fingerprint(&slower), baseline);
    }

    #[test]
    fn a_different_verdict_is_a_different_fingerprint() {
        let baseline = manifest_fingerprint(&manifest());
        let mut failed = manifest();
        failed.checks[0].status = CheckStatus::Failed { exit_code: 101 };
        assert_ne!(manifest_fingerprint(&failed), baseline);

        let mut breached = manifest();
        breached.breaches.push(ScopeBreach {
            path: PathBuf::from("src/lib.rs"),
            reason: "outside the declared scope".into(),
        });
        assert_ne!(manifest_fingerprint(&breached), baseline);
    }

    fn plan(changes: Vec<WorkspaceChange>) -> VerificationPlan {
        VerificationPlan {
            work_order_id: "wo-1".into(),
            goal: "g".into(),
            scope: vec![],
            mutated: vec![],
            changes,
            workspace: Default::default(),
            witnesses: vec![],
            acceptance: vec![],
        }
    }

    #[test]
    fn changing_nothing_has_no_diff_at_all() {
        assert_eq!(diff_fingerprint(&plan(vec![])), None);
    }

    #[test]
    fn the_diff_is_the_paths_and_what_they_ended_holding() {
        let present = |path: &str, hash: &str| WorkspaceChange {
            path: PathBuf::from(path),
            state: ChangeState::Present {
                content_hash: hash.into(),
            },
        };
        let one = diff_fingerprint(&plan(vec![present("a.rs", "h1"), present("b.rs", "h2")]));
        // Discovery order is not part of the change set.
        let other = diff_fingerprint(&plan(vec![present("b.rs", "h2"), present("a.rs", "h1")]));
        assert_eq!(one, other);

        let deleted = diff_fingerprint(&plan(vec![
            present("a.rs", "h1"),
            WorkspaceChange {
                path: PathBuf::from("b.rs"),
                state: ChangeState::Missing,
            },
        ]));
        assert_ne!(deleted, one, "a deletion is not the same as a rewrite");
    }
}
