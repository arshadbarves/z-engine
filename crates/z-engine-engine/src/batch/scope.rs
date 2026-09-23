//! The policy decision for one action. A worktree agent is decided against
//! a policy rooted at its worktree; the main project stays readable under
//! the session policy, and changes to it outside the worktree are refused.

use std::path::{Path, PathBuf};

use z_engine_policy::{Action, Decision};

use crate::run::RunContext;
use crate::settings::build_policy;
use crate::sync::lock;

pub(super) fn decide(ctx: &RunContext, tool: &str, action: &Action) -> Decision {
    let mode = ctx.mode();
    let core = &ctx.core;
    let Some(scope) = &ctx.spec.worktree else {
        return lock(&core.policy).decide(tool, action, mode);
    };
    let worktree = &ctx.spec.root;
    let in_project = |path: &PathBuf| path.starts_with(&scope.project) && !inside(worktree, path);
    match action {
        Action::Write { paths } if paths.iter().any(in_project) => Decision::Deny {
            reason: format!(
                "this agent works in the isolated worktree {}; it may change files only there",
                worktree.display()
            ),
        },
        Action::Read { paths } if !paths.is_empty() && paths.iter().all(in_project) => {
            lock(&core.policy).decide(tool, action, mode)
        }
        _ => {
            let granted = lock(&core.policy).session_rules();
            let home = core.shared.paths.home_dir.as_deref();
            let (policy, _) = build_policy(&core.settings(), worktree, home, &granted);
            policy.decide(tool, action, mode)
        }
    }
}

fn inside(dir: &Path, path: &Path) -> bool {
    path.starts_with(dir)
}
