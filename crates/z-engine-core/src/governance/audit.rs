//! Reconciling three accounts of a guarded run: what the harness
//! authorized, what the workspace shows, and what the run read.
//!
//! Kept apart from [`super::verify`] because it is the one part of
//! verification that is pure judgement over already-gathered facts plus a
//! re-read of the files in question — no processes, no timeouts, no
//! policy about commands. It is also the part most worth reading on its
//! own, because it is what makes the mutation log non-self-certifying:
//! the log says what the governed tools did, the snapshot says what
//! actually changed, and only their exact agreement is authorized.
//!
//! [`reconcile_after_checks`] closes the other end of the same question:
//! the audit judges the tree before the checks run, so the tree is
//! captured again afterwards and anything that moved in between is
//! either the harness's own doing or a breach.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::evidence::BlobHandle;

use super::manifest::ScopeBreach;
use super::plan::{ChangeState, VerificationPlan, WorkspaceChange};
use super::snapshot::WorkspaceSnapshot;

/// Everything the declared scope and the mutation log fail to account for.
///
/// Four questions, each of which can only be answered by looking outside
/// the log that claims the answer:
///
/// 1. did every authorized write stay inside the declared scope?
/// 2. does every authorized write still hold the bytes it authorized —
///    or did something else overwrite, revert, or delete it afterwards?
/// 3. is every change the workspace shows one of those authorized writes?
/// 4. does everything this run read outside its scope still say what it
///    said when it was read?
pub(super) fn audit(root: &Path, plan: &VerificationPlan) -> Vec<ScopeBreach> {
    let mut breaches = Vec::new();
    let authorized: BTreeSet<&PathBuf> = plan.mutated.iter().map(|m| &m.path).collect();

    for record in &plan.mutated {
        if !plan.scope.contains(&record.path) {
            breaches.push(ScopeBreach {
                path: record.path.clone(),
                reason: "changed but not declared writable".into(),
            });
        }
    }

    for path in &authorized {
        let expected = plan.authorized_hash(path).unwrap_or_default();
        let reason = match std::fs::read(root.join(path)) {
            Ok(bytes) if BlobHandle::of(&bytes).to_string() == expected => continue,
            Ok(_) => {
                "no longer holds the bytes this run wrote, so something outside this run changed \
                 it afterwards"
            }
            Err(_) => "was written by this run and is no longer readable where it was written",
        };
        breaches.push(ScopeBreach {
            path: (*path).clone(),
            reason: reason.into(),
        });
    }

    for change in &plan.changes {
        if authorized.contains(&change.path) {
            continue; // already judged against the hash it was authorized for
        }
        let reason = match change.state {
            ChangeState::Present { .. } => {
                "changed during this run, but no governed tool recorded the change"
            }
            ChangeState::Missing => {
                "disappeared during this run, but no governed tool recorded the change"
            }
        };
        breaches.push(ScopeBreach {
            path: change.path.clone(),
            reason: reason.into(),
        });
    }

    for witness in &plan.witnesses {
        if plan.scope.contains(&witness.path) || authorized.contains(&witness.path) {
            continue;
        }
        let reason = match std::fs::read(root.join(&witness.path)) {
            Ok(bytes) if BlobHandle::of(&bytes).to_string() == witness.file_hash => continue,
            Ok(_) => "changed since this run read it, outside the declared scope",
            Err(_) => "was read by this run and is now unreadable",
        };
        breaches.push(ScopeBreach {
            path: witness.path.clone(),
            reason: reason.into(),
        });
    }

    breaches.sort_by(|a, b| a.path.cmp(&b.path));
    breaches.dedup_by(|a, b| a.path == b.path);
    breaches
}

/// Files the harness's own toolchain writes as a side effect of being
/// asked a question. Cargo refreshes the lockfile whenever it resolves a
/// dependency graph, including for `check` and `test`.
///
/// Deliberately a closed list of *file names*, not a prefix or a
/// pattern: anything broader would let an acceptance command hide a
/// source edit behind a plausible-looking path.
const HARNESS_OWNED: &[&str] = &["Cargo.lock"];

/// What running the checks left behind.
pub(super) struct Residue {
    /// Changes the harness itself is answerable for. The next turn is
    /// not asked to explain these.
    pub harness: Vec<WorkspaceChange>,
    /// Changes nothing in the harness accounts for. These arrived after
    /// the audit that judged this run, so nothing has judged them.
    pub breaches: Vec<ScopeBreach>,
}

/// The audit judges the workspace *before* the checks run, because a
/// check must not be trusted to describe the tree it is about to touch.
/// That leaves a window: an acceptance command could edit sources after
/// the audit passed and before the manifest is written.
///
/// So the tree is captured again afterwards and the difference is split
/// in two: the lockfile cargo refreshes, which no agent wrote and no
/// later turn can be asked about, and everything else — which blocks.
pub(super) fn reconcile_after_checks(pre: &WorkspaceSnapshot, post: &WorkspaceSnapshot) -> Residue {
    let mut residue = Residue {
        harness: Vec::new(),
        breaches: Vec::new(),
    };
    for change in pre.changes(post) {
        let owned = change
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| HARNESS_OWNED.contains(&name));
        if owned {
            residue.harness.push(change);
            continue;
        }
        residue.breaches.push(ScopeBreach {
            path: change.path,
            reason: "changed while the completion checks were running, after the audit that \
                     judged this run"
                .into(),
        });
    }
    residue
}

#[cfg(test)]
mod tests;
