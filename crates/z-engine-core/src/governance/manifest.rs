//! `VerificationManifest`: the typed record of what a guarded run proved
//! before it was allowed to call itself done, and the verdict derived
//! from it.
//!
//! Pure data and pure rules — nothing here spawns a process or touches
//! the filesystem; [`super::verify`] gathers the outcomes and this module
//! decides what they mean. The split matters because the verdict is the
//! load-bearing part: a manifest is *complete* only when every required
//! check actually ran and passed, so an absent check reads as a refusal
//! rather than as an absence of bad news.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How one check ended. Every non-`Passed` variant carries why, because
/// the text reaches both the model and the UI verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum CheckStatus {
    Passed,
    Failed {
        exit_code: i32,
    },
    TimedOut {
        after_secs: u64,
    },
    /// The command could not be executed at all (program missing, spawn
    /// refused). Distinct from `Failed`: nothing was proven either way.
    Unavailable {
        reason: String,
    },
    /// Refused before execution — the command is not one this harness
    /// will run as verification evidence.
    Rejected {
        reason: String,
    },
    /// Deliberately not required here, with the reason recorded so the
    /// manifest never looks like it silently skipped something.
    Skipped {
        reason: String,
    },
}

impl CheckStatus {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// One-line explanation of a non-pass, for the refusal text.
    fn detail(&self) -> String {
        match self {
            Self::Passed => "passed".into(),
            Self::Failed { exit_code } => format!("failed (exit {exit_code})"),
            Self::TimedOut { after_secs } => format!("timed out after {after_secs}s"),
            Self::Unavailable { reason } => format!("could not run: {reason}"),
            Self::Rejected { reason } => format!("refused: {reason}"),
            Self::Skipped { reason } => format!("skipped: {reason}"),
        }
    }
}

/// One check and what it proved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckOutcome {
    /// Stable label (`cargo-check`, `acceptance`) for machine consumers.
    pub name: String,
    /// The command line as it was requested.
    pub command: String,
    /// Whether completion depends on this check passing.
    pub required: bool,
    pub status: CheckStatus,
    pub duration_ms: u64,
    /// Bounded tail of the combined output, for the failure message.
    pub output_tail: String,
}

impl CheckOutcome {
    /// A check that never ran because there was nothing to run it on.
    pub fn skipped(name: &str, command: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            required: false,
            status: CheckStatus::Skipped {
                reason: reason.into(),
            },
            duration_ms: 0,
            output_tail: String::new(),
        }
    }

    fn headline(&self) -> String {
        format!("`{}` {}", self.command, self.status.detail())
    }
}

/// A file that changed when the declared scope said it would not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeBreach {
    pub path: PathBuf,
    pub reason: String,
}

/// Everything a guarded run offers as proof that its work is done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationManifest {
    pub work_order_id: String,
    pub goal: String,
    /// Repository-relative paths the order declared writable.
    pub scope: Vec<PathBuf>,
    /// Repository-relative paths this run actually changed.
    pub mutated: Vec<PathBuf>,
    /// Changes the declared scope does not account for.
    pub breaches: Vec<ScopeBreach>,
    pub checks: Vec<CheckOutcome>,
}

/// What the manifest permits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every required check ran and passed, and nothing changed outside
    /// the declared scope.
    Complete,
    /// Completion is refused; the string is the model/UI-facing reason.
    Blocked(String),
}

impl VerificationManifest {
    /// The verdict, in the order a reviewer would reach it: scope first
    /// (a change nobody authorized invalidates every later check), then
    /// the presence of evidence, then the evidence itself.
    pub fn verdict(&self) -> Verdict {
        if !self.breaches.is_empty() {
            let listed = self
                .breaches
                .iter()
                .map(|b| format!("{} ({})", b.path.display(), b.reason))
                .collect::<Vec<_>>()
                .join(", ");
            return Verdict::Blocked(format!(
                "changes outside the declared scope of work order {}: {listed}",
                self.work_order_id
            ));
        }
        let required: Vec<&CheckOutcome> = self.checks.iter().filter(|c| c.required).collect();
        if required.is_empty() {
            return Verdict::Blocked(
                "nothing was verified: this run changed files but produced no required check"
                    .into(),
            );
        }
        let failed: Vec<String> = required
            .iter()
            .filter(|c| !c.status.is_pass())
            .map(|c| c.headline())
            .collect();
        if failed.is_empty() {
            Verdict::Complete
        } else {
            Verdict::Blocked(format!("verification did not pass: {}", failed.join("; ")))
        }
    }

    pub fn is_complete(&self) -> bool {
        self.verdict() == Verdict::Complete
    }

    /// Multi-line restatement for the transcript: what ran, what it said,
    /// and the tail of any failing output.
    pub fn summary(&self) -> String {
        let mut out = format!("verification for work order {}\n", self.work_order_id);
        for breach in &self.breaches {
            out.push_str(&format!(
                "- out of scope: {} ({})\n",
                breach.path.display(),
                breach.reason
            ));
        }
        for check in &self.checks {
            out.push_str(&format!(
                "- {} {}\n",
                check.headline(),
                if check.required { "[required]" } else { "" }
            ));
            if !check.status.is_pass() && !check.output_tail.is_empty() {
                for line in check
                    .output_tail
                    .lines()
                    .rev()
                    .take(20)
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                {
                    out.push_str(&format!("    {line}\n"));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, required: bool, status: CheckStatus) -> CheckOutcome {
        CheckOutcome {
            name: name.into(),
            command: format!("cargo {name}"),
            required,
            status,
            duration_ms: 1,
            output_tail: "error[E0433]: failed to resolve".into(),
        }
    }

    fn manifest(checks: Vec<CheckOutcome>, breaches: Vec<ScopeBreach>) -> VerificationManifest {
        VerificationManifest {
            work_order_id: "wo-1".into(),
            goal: "make parse fallible".into(),
            scope: vec![PathBuf::from("src/lib.rs")],
            mutated: vec![PathBuf::from("src/lib.rs")],
            breaches,
            checks,
        }
    }

    #[test]
    fn a_manifest_whose_required_checks_all_passed_is_complete() {
        let m = manifest(vec![check("check", true, CheckStatus::Passed)], vec![]);
        assert_eq!(m.verdict(), Verdict::Complete);
        assert!(m.is_complete());
    }

    #[test]
    fn a_failing_required_check_blocks_and_names_itself() {
        let m = manifest(
            vec![check("check", true, CheckStatus::Failed { exit_code: 101 })],
            vec![],
        );
        let Verdict::Blocked(reason) = m.verdict() else {
            panic!("a failing check cannot complete");
        };
        assert!(
            reason.contains("cargo check") && reason.contains("exit 101"),
            "{reason}"
        );
    }

    #[test]
    fn a_timeout_and_an_unrunnable_command_both_block() {
        for status in [
            CheckStatus::TimedOut { after_secs: 30 },
            CheckStatus::Unavailable {
                reason: "no such file".into(),
            },
            CheckStatus::Rejected {
                reason: "not allowlisted".into(),
            },
        ] {
            let m = manifest(vec![check("test", true, status.clone())], vec![]);
            assert!(!m.is_complete(), "{status:?} must not complete");
        }
    }

    /// The absence of bad news is not good news: a run that produced no
    /// required check has proven nothing at all.
    #[test]
    fn a_manifest_without_a_required_check_is_never_complete() {
        let m = manifest(vec![check("check", false, CheckStatus::Passed)], vec![]);
        let Verdict::Blocked(reason) = m.verdict() else {
            panic!("an unverified run cannot complete");
        };
        assert!(reason.contains("nothing was verified"), "{reason}");
        assert!(!manifest(vec![], vec![]).is_complete());
    }

    /// A change nobody authorized invalidates the run regardless of what
    /// the compiler thinks of it.
    #[test]
    fn an_out_of_scope_change_blocks_even_when_every_check_passed() {
        let m = manifest(
            vec![check("check", true, CheckStatus::Passed)],
            vec![ScopeBreach {
                path: PathBuf::from("src/other.rs"),
                reason: "changed since it was read".into(),
            }],
        );
        let Verdict::Blocked(reason) = m.verdict() else {
            panic!("an out-of-scope change cannot complete");
        };
        assert!(reason.contains("src/other.rs"), "{reason}");
    }

    #[test]
    fn the_summary_states_every_check_and_the_failing_tail() {
        let m = manifest(
            vec![check("check", true, CheckStatus::Failed { exit_code: 1 })],
            vec![],
        );
        let s = m.summary();
        assert!(s.contains("work order wo-1"));
        assert!(s.contains("`cargo check` failed (exit 1) [required]"));
        assert!(s.contains("error[E0433]"));
    }

    #[test]
    fn the_manifest_round_trips_as_json() {
        let m = manifest(vec![check("check", true, CheckStatus::Passed)], vec![]);
        let text = serde_json::to_string_pretty(&m).unwrap();
        assert_eq!(
            serde_json::from_str::<VerificationManifest>(&text).unwrap(),
            m
        );
        assert!(text.contains("\"workOrderId\""));
    }
}
