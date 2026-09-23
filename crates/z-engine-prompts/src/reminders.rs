//! System reminders injected between rounds, and the environment template.
//!
//! Templates use `{{name}}` placeholders; a line holding an optional
//! `{{name?}}` placeholder is dropped when its value is empty.
//! `z-engine-context` renders them and wraps reminders in
//! `<system-reminder>` tags.

/// Environment block of the system prompt.
pub const ENVIRONMENT: &str = include_str!("../prompts/reminders/environment.md");

/// Git snapshot embedded in [`ENVIRONMENT`] for git repositories.
pub const GIT_SNAPSHOT: &str = include_str!("../prompts/reminders/git-snapshot.md");

/// Preamble placed before the project and user instruction files.
pub const INSTRUCTIONS_PREAMBLE: &str =
    include_str!("../prompts/reminders/instructions-preamble.md");

/// Skills section of the system prompt; `{{skills}}` holds one line per skill.
pub const SKILLS_LISTING: &str = include_str!("../prompts/reminders/skills-listing.md");

/// The todo list is empty; suggest `TodoWrite` for multi-step work.
pub const TODO_NUDGE: &str = include_str!("../prompts/reminders/todo-nudge.md");

/// The current todo list.
pub const TODO_STATE: &str = include_str!("../prompts/reminders/todo-state.md");

/// Plan mode rules: read-only research, then `ExitPlanMode`.
pub const PLAN_MODE_ACTIVE: &str = include_str!("../prompts/reminders/plan-mode-active.md");

/// The user approved the plan; implementation may start.
pub const PLAN_APPROVED: &str = include_str!("../prompts/reminders/plan-approved.md");

/// Files changed on disk since the model last read them.
pub const FILES_CHANGED_EXTERNALLY: &str =
    include_str!("../prompts/reminders/files-changed-externally.md");

/// Instruction files found in directories the model just accessed.
pub const NESTED_INSTRUCTIONS: &str = include_str!("../prompts/reminders/nested-instructions.md");

/// A background shell or agent job finished.
pub const JOB_FINISHED: &str = include_str!("../prompts/reminders/job-finished.md");

/// Messages the user sent while the model was working.
pub const STEERING: &str = include_str!("../prompts/reminders/steering.md");

/// Changed files are unverified under `auto` or `strict` verification.
pub const VERIFICATION_REQUIRED: &str =
    include_str!("../prompts/reminders/verification-required.md");

/// Checks run automatically at the stop boundary failed.
pub const AUTO_CHECK_FAILED: &str = include_str!("../prompts/reminders/auto-check-failed.md");

/// The user interrupted the previous response.
pub const INTERRUPTED: &str = include_str!("../prompts/reminders/interrupted.md");

/// User message that replaces summarized history after compaction.
pub const COMPACTION_SUMMARY: &str = include_str!("../prompts/reminders/compaction-summary.md");
