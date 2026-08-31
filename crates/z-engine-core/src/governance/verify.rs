//! `VerificationRunner`: the gate that stands between a guarded run's
//! *claim* of completion and the turn actually ending.
//!
//! Everything else in `governance` is pure; this is its one impure edge,
//! and it is deliberately narrow. It re-checks the evidence hashes the run
//! captured (so a change nobody authorized is caught even though it never
//! passed the mutation gate), runs the workspace compile check, runs each
//! declared acceptance command through [`super::command_run`], and returns
//! a [`VerificationManifest`]. It decides nothing: the verdict is the
//! manifest's, and completion is the caller's to grant only on a complete
//! one.
//!
//! Facts come in through [`VerificationPlan`] — the runner never asks the
//! tools layer what changed, never canonicalizes a path, and never hashes
//! anything but the bytes currently on disk (using the `evidence` module's
//! own hash, so "changed" means exactly what it meant at capture time).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::evidence::BlobHandle;

use super::command_run::{run_bounded, tail};
use super::manifest::{CheckOutcome, CheckStatus, ScopeBreach, Verdict, VerificationManifest};
use super::work_order::AcceptanceCommand;

/// Default wall-clock bound per check. Generous enough for a cold
/// workspace compile, finite enough that a hung test suite cannot hold a
/// turn open forever.
pub const DEFAULT_CHECK_TIMEOUT: Duration = Duration::from_secs(600);

/// Programs verification will execute. Narrow on purpose: these are run
/// without an approval prompt, so the list is the harness's own promise
/// about what a work order can make it do.
const DEFAULT_ALLOWED_PROGRAMS: &[&str] = &["cargo"];

const CARGO_CHECK: &str = "cargo check --workspace --all-targets --message-format=json";

/// One path this run read, and the hash it had when it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadWitness {
    /// Repository-relative path, as recorded on the evidence record.
    pub path: PathBuf,
    /// SHA-256 of the whole file at capture time.
    pub file_hash: String,
}

/// The facts a verification run is judged against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPlan {
    pub work_order_id: String,
    pub goal: String,
    /// Repository-relative writable paths the order declared.
    pub scope: Vec<PathBuf>,
    /// Repository-relative paths this run actually changed.
    pub mutated: Vec<PathBuf>,
    /// Every path this run read, with the hash it had at read time.
    pub witnesses: Vec<ReadWitness>,
    pub acceptance: Vec<AcceptanceCommand>,
}

/// Runs the checks a guarded completion depends on.
#[derive(Debug, Clone)]
pub struct VerificationRunner {
    root: PathBuf,
    timeout: Duration,
    allowed: Vec<String>,
    abort: Option<Arc<AtomicBool>>,
}

impl VerificationRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            timeout: DEFAULT_CHECK_TIMEOUT,
            allowed: DEFAULT_ALLOWED_PROGRAMS
                .iter()
                .map(|p| (*p).to_string())
                .collect(),
            abort: None,
        }
    }

    /// Share the run's cooperative abort flag, so stopping the turn stops
    /// the checks instead of waiting out their timeouts.
    pub fn with_abort(mut self, abort: Arc<AtomicBool>) -> Self {
        self.abort = Some(abort);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replace the program allowlist. Tests use this to reach the timeout
    /// and missing-program paths without pretending `cargo` misbehaves.
    pub fn with_allowed_programs(mut self, programs: &[&str]) -> Self {
        self.allowed = programs.iter().map(|p| (*p).to_string()).collect();
        self
    }

    /// Verify `plan` and return what was proven.
    pub async fn run(&self, plan: &VerificationPlan) -> VerificationManifest {
        // Audit before running anything. The checks are processes that
        // write to the workspace themselves — `cargo check` refreshes
        // `Cargo.lock`, a test suite touches `target/` — and auditing
        // afterwards would charge those writes to the agent as scope
        // breaches it did not commit.
        let breaches = self.audit_scope(plan);
        let mut checks = vec![self.cargo_check(plan).await];
        checks.extend(self.acceptance_checks(&plan.acceptance).await);
        VerificationManifest {
            work_order_id: plan.work_order_id.clone(),
            goal: plan.goal.clone(),
            scope: plan.scope.clone(),
            mutated: plan.mutated.clone(),
            breaches,
            checks,
        }
    }

    /// Changes the declared scope does not account for, from two
    /// directions: something changed that was never declared writable, and
    /// something the run read outside its scope no longer matches the
    /// bytes it read.
    fn audit_scope(&self, plan: &VerificationPlan) -> Vec<ScopeBreach> {
        let mut breaches = Vec::new();
        for path in &plan.mutated {
            if !plan.scope.contains(path) {
                breaches.push(ScopeBreach {
                    path: path.clone(),
                    reason: "changed but not declared writable".into(),
                });
            }
        }
        for witness in &plan.witnesses {
            if plan.scope.contains(&witness.path) || plan.mutated.contains(&witness.path) {
                continue;
            }
            let reason = match std::fs::read(self.root.join(&witness.path)) {
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

    /// Does the workspace still compile?
    ///
    /// Required whenever this run touched Rust — and if it touched Rust
    /// with no manifest anywhere to compile against, that is a refusal,
    /// not a skip: the alternative would leave the order's own acceptance
    /// commands as the only required evidence, which is the model
    /// grading its own work.
    async fn cargo_check(&self, plan: &VerificationPlan) -> CheckOutcome {
        let touched_rust = plan.mutated.iter().any(|p| is_rust_relevant(p));
        let Some(dir) = self.cargo_root(plan) else {
            return match touched_rust {
                false => CheckOutcome::skipped(
                    "cargo-check",
                    CARGO_CHECK,
                    "no Cargo.toml at the project root, and this run changed no Rust",
                ),
                true => CheckOutcome {
                    name: "cargo-check".into(),
                    command: CARGO_CHECK.into(),
                    required: true,
                    status: CheckStatus::Unavailable {
                        reason: "this run changed Rust sources but no Cargo.toml was found to \
                                 compile them against"
                            .into(),
                    },
                    duration_ms: 0,
                    output_tail: String::new(),
                },
            };
        };
        let run = run_bounded(CARGO_CHECK, &dir, self.timeout, &self.allowed, self.abort()).await;
        // The exit status is authoritative — a manifest error emits no
        // compiler messages at all — but when cargo did produce
        // diagnostics they explain the failure far better than raw JSON.
        let output_tail = match run.status.is_pass() {
            true => String::new(),
            false => render_diagnostics(&run.stdout).unwrap_or(run.output_tail),
        };
        CheckOutcome {
            name: "cargo-check".into(),
            command: CARGO_CHECK.into(),
            required: true,
            status: run.status,
            duration_ms: run.duration_ms,
            output_tail,
        }
    }

    /// Every acceptance command the order declared. A mutating order that
    /// declared none has offered no proof of its own goal, which is a
    /// refusal recorded as such rather than an empty list.
    async fn acceptance_checks(&self, commands: &[AcceptanceCommand]) -> Vec<CheckOutcome> {
        if commands.is_empty() {
            return vec![CheckOutcome {
                name: "acceptance".into(),
                command: "(none declared)".into(),
                required: true,
                status: CheckStatus::Rejected {
                    reason: "the work order declares no acceptance command, so nothing proves its \
                             goal was met"
                        .into(),
                },
                duration_ms: 0,
                output_tail: String::new(),
            }];
        }
        let mut out = Vec::with_capacity(commands.len());
        for command in commands {
            let run = run_bounded(
                &command.command,
                &self.root,
                self.timeout,
                &self.allowed,
                self.abort(),
            )
            .await;
            out.push(CheckOutcome {
                name: "acceptance".into(),
                command: command.command.clone(),
                required: true,
                status: run.status,
                duration_ms: run.duration_ms,
                output_tail: run.output_tail,
            });
        }
        out
    }

    fn abort(&self) -> Option<&AtomicBool> {
        self.abort.as_deref()
    }

    /// Where to run `cargo check`: the project root when it is a cargo
    /// root, otherwise the nearest enclosing manifest above a Rust file
    /// this run changed. Never escapes the project root.
    fn cargo_root(&self, plan: &VerificationPlan) -> Option<PathBuf> {
        if self.root.join("Cargo.toml").is_file() {
            return Some(self.root.clone());
        }
        plan.mutated
            .iter()
            .filter(|p| is_rust_relevant(p))
            .find_map(|p| {
                let mut dir = self.root.join(p);
                while dir.pop() && dir.starts_with(&self.root) {
                    if dir.join("Cargo.toml").is_file() {
                        return Some(dir);
                    }
                }
                None
            })
    }
}

/// Whether changing this path can change what `cargo check` says.
fn is_rust_relevant(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    path.extension().is_some_and(|e| e == "rs") || name == "Cargo.toml" || name == "Cargo.lock"
}

/// Render cargo's JSON diagnostics into the manifest tail, reusing the
/// same decoder the post-edit hook uses. `None` when cargo emitted no
/// compiler messages, so the caller keeps the raw output instead.
fn render_diagnostics(stdout: &str) -> Option<String> {
    let diags = crate::lsp::cargo_check::parse(stdout);
    let rendered: Vec<String> = diags
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| format!("{}:{} {} [{}]", d.file, d.line, d.message, d.code))
        .collect();
    (!rendered.is_empty()).then(|| tail(&rendered.join("\n")))
}

/// Persist the manifest beside the run's other evidence, alongside the
/// verdict it produced. The verdict is written rather than left implicit
/// so anyone reading the file — a human, a later run, a CI step — sees
/// what it decided without re-implementing the rule.
pub fn write_manifest(dir: &Path, manifest: &VerificationManifest) -> std::io::Result<PathBuf> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Persisted<'a> {
        #[serde(flatten)]
        manifest: &'a VerificationManifest,
        complete: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_reason: Option<String>,
    }

    std::fs::create_dir_all(dir)?;
    let path = dir.join("verification.json");
    let blocked_reason = match manifest.verdict() {
        Verdict::Complete => None,
        Verdict::Blocked(reason) => Some(reason),
    };
    let mut text = serde_json::to_string_pretty(&Persisted {
        manifest,
        complete: blocked_reason.is_none(),
        blocked_reason,
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    text.push('\n');
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests;
