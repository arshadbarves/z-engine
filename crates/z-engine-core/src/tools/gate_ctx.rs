//! The tools-layer adapter for the governance mutation gate: gathers the
//! facts [`crate::governance::gate`] needs and applies its verdict.
//!
//! Thin by design. Every fact comes from something that already owns it —
//! canonical path identity and evidence freshness from [`ToolCtx`]
//! (Task 3), the active order from the work-order store (Task 4), Rust
//! symbols from the repo map's tree-sitter outline, and semantic health
//! from the language server behind [`RustSemantics`]. Nothing here
//! re-implements hashing, path normalization, or symbol discovery, and
//! none of the *rules* live here: they live in the pure gate.

use std::path::Path;

use crate::evidence::{BlobHandle, EvidenceRecord};
use crate::governance::{
    EvidenceState, GateEngine, GateFailure, LineRange, MutationRequest, RustFacts, SemanticHealth,
};
use crate::lsp::LspHealth;
use crate::perms::PolicyEngine;

use super::ToolCtx;
use super::path_identity::{canonical_in_root, canonicalize_root, to_repo_relative};

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
        let identity = self.gate_identity(path);
        let rust = match is_rust(path) {
            false => None,
            true => Some(RustFacts {
                health: semantic_health(self.semantic_health().await),
                declared: declared_symbols(current),
            }),
        };
        GateEngine::authorize(&MutationRequest {
            path,
            identity: identity.as_deref(),
            order: order.as_deref(),
            changed,
            evidence: self.evidence_state(path, current),
            rust,
        })
        .into_result()
    }

    /// Authorize one shell command. Guarded runs only run commands whose
    /// write set is provably empty; session prefix rules deliberately do
    /// not count, since they authorize *approval*, not proof.
    pub fn authorize_command(&self, command: &str) -> Result<(), GateFailure> {
        if self.work_orders.is_none() {
            return Ok(());
        }
        GateEngine::authorize_command(command, PolicyEngine::is_provably_read_only(command))
            .into_result()
    }

    /// Canonical repository-relative identity, reusing the same rules that
    /// admitted the work order (`EvidenceView::repo_relative_identity`).
    fn gate_identity(&self, path: &Path) -> Option<String> {
        let canonical = canonical_in_root(&self.resolve(path), &self.project_root)?;
        Some(to_repo_relative(
            &canonical,
            &canonicalize_root(&self.project_root),
        ))
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

/// Rust source is the only content this slice makes semantic claims about.
fn is_rust(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
}

/// Symbols declared in `bytes`, via the repo map's tree-sitter outline —
/// the project's one Rust symbol extractor. Non-UTF-8 or unparseable
/// content declares nothing, which blocks rather than passes.
fn declared_symbols(bytes: &[u8]) -> Vec<String> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    crate::context::repo_map::extract_rust(text)
        .map(|outline| outline.symbols.into_iter().map(|s| s.name).collect())
        .unwrap_or_default()
}

fn semantic_health(health: LspHealth) -> SemanticHealth {
    match health {
        LspHealth::Ready => SemanticHealth::Ready,
        LspHealth::Unavailable(reason) => SemanticHealth::Unavailable { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::gate::GateFailure;
    use crate::governance::{AcceptanceCommand, WorkOrder};
    use crate::lsp::LspHealth;
    use crate::tools::test_support::{guarded_ctx, plain_ctx};
    use std::path::{Path, PathBuf};

    const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";

    fn order(paths: &[&str], symbols: &[&str], evidence: &[&str]) -> WorkOrder {
        WorkOrder {
            id: "wo-1".into(),
            goal: "make parse fallible".into(),
            writable_paths: paths.iter().map(PathBuf::from).collect(),
            target_symbols: symbols.iter().map(|s| (*s).to_string()).collect(),
            evidence_ids: evidence.iter().map(|s| (*s).to_string()).collect(),
            acceptance_commands: vec![AcceptanceCommand {
                command: "cargo test".into(),
                description: "unit tests".into(),
            }],
        }
    }

    /// Record a whole-file read of `name` exactly like `read_file` does.
    fn read(ctx: &ToolCtx, name: &str, bytes: &[u8]) -> String {
        ctx.record_read_evidence(&ctx.resolve(Path::new(name)), None, bytes, bytes)
            .unwrap()
            .expect("in-root read must be recorded")
    }

    /// A guarded run that has read `lib.rs` and declared an order over it.
    fn ready(health: LspHealth) -> (ToolCtx, tempfile::TempDir, tempfile::TempDir) {
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), LIB).unwrap();
        let (ctx, store) = guarded_ctx(repo.path(), Some(health));
        let id = read(&ctx, "lib.rs", LIB.as_bytes());
        ctx.set_work_order(&order(&["lib.rs"], &["parse"], &[&id]))
            .unwrap();
        (ctx, store, repo)
    }

    #[tokio::test]
    async fn unguarded_runs_authorize_every_mutation_and_command() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = plain_ctx(tmp.path());
        assert!(
            ctx.authorize_mutation(Path::new("anything.rs"), b"x\n", Some((1, 1)))
                .await
                .is_ok()
        );
        assert!(ctx.authorize_command("rm -rf build").is_ok());
    }

    #[tokio::test]
    async fn guarded_runs_refuse_mutations_before_an_order_is_declared() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), LIB).unwrap();
        let (ctx, _store) = guarded_ctx(repo.path(), Some(LspHealth::Ready));
        read(&ctx, "lib.rs", LIB.as_bytes());

        let err = ctx
            .authorize_mutation(
                &ctx.resolve(Path::new("lib.rs")),
                LIB.as_bytes(),
                Some((1, 1)),
            )
            .await
            .unwrap_err();
        assert_eq!(err, GateFailure::NoWorkOrder);
    }

    #[tokio::test]
    async fn guarded_runs_authorize_a_scoped_evidence_backed_rust_edit() {
        let (ctx, _store, repo) = ready(LspHealth::Ready);
        ctx.authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
            .await
            .expect("scoped, evidence-backed, localized edit must pass");
    }

    #[tokio::test]
    async fn guarded_runs_refuse_paths_outside_the_declared_scope() {
        let (ctx, _store, repo) = ready(LspHealth::Ready);
        std::fs::write(repo.path().join("other.rs"), LIB).unwrap();
        let err = ctx
            .authorize_mutation(&repo.path().join("other.rs"), LIB.as_bytes(), Some((1, 1)))
            .await
            .unwrap_err();
        assert!(matches!(err, GateFailure::OutOfScope { .. }), "{err}");
    }

    #[tokio::test]
    async fn guarded_runs_refuse_bytes_that_no_longer_match_the_read() {
        let (ctx, _store, repo) = ready(LspHealth::Ready);
        // The bytes about to be modified are not the bytes that were read.
        let err = ctx
            .authorize_mutation(
                &repo.path().join("lib.rs"),
                b"pub fn parse() {}\n",
                Some((1, 1)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, GateFailure::StaleEvidence { .. }), "{err}");
    }

    #[tokio::test]
    async fn guarded_runs_refuse_rust_edits_without_a_healthy_semantic_provider() {
        let (ctx, _store, repo) = ready(LspHealth::Unavailable("spawn failed".into()));
        let err = ctx
            .authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
            .await
            .unwrap_err();
        assert!(
            matches!(err, GateFailure::SemanticProviderUnavailable { .. }),
            "{err}"
        );

        // No provider attached at all is the same answer, never a pass.
        let repo2 = tempfile::tempdir().unwrap();
        std::fs::write(repo2.path().join("lib.rs"), LIB).unwrap();
        let (bare, _s2) = guarded_ctx(repo2.path(), None);
        let id = read(&bare, "lib.rs", LIB.as_bytes());
        bare.set_work_order(&order(&["lib.rs"], &["parse"], &[&id]))
            .unwrap();
        let err = bare
            .authorize_mutation(&repo2.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
            .await
            .unwrap_err();
        assert!(
            matches!(err, GateFailure::SemanticProviderUnavailable { .. }),
            "{err}"
        );
    }

    #[tokio::test]
    async fn guarded_runs_refuse_rust_edits_whose_target_symbol_is_absent() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), LIB).unwrap();
        let (ctx, _store) = guarded_ctx(repo.path(), Some(LspHealth::Ready));
        let id = read(&ctx, "lib.rs", LIB.as_bytes());
        ctx.set_work_order(&order(&["lib.rs"], &["render"], &[&id]))
            .unwrap();

        let err = ctx
            .authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
            .await
            .unwrap_err();
        assert!(
            matches!(err, GateFailure::UnresolvedTargetSymbol { .. }),
            "{err}"
        );
    }

    #[tokio::test]
    async fn non_rust_files_in_scope_need_no_semantic_provider() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(repo.path().join("notes.md"), "# notes\n").unwrap();
        let (ctx, _store) = guarded_ctx(repo.path(), None);
        let id = read(&ctx, "notes.md", b"# notes\n");
        ctx.set_work_order(&order(&["notes.md"], &[], &[&id]))
            .unwrap();

        ctx.authorize_mutation(&repo.path().join("notes.md"), b"# notes\n", Some((1, 1)))
            .await
            .expect("markdown carries no Rust semantic claim");
    }

    #[test]
    fn guarded_runs_allow_provably_read_only_commands_and_refuse_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _store) = guarded_ctx(tmp.path(), Some(LspHealth::Ready));
        assert!(ctx.authorize_command("ls -la").is_ok());
        assert!(ctx.authorize_command("git status").is_ok());
        let err = ctx.authorize_command("rm -rf build").unwrap_err();
        assert!(matches!(err, GateFailure::UnprovenWriteSet { .. }), "{err}");
        // Session prefix rules must not launder a mutation past the gate.
        ctx.perms
            .lock()
            .unwrap()
            .add_session_rule("cargo test*".into());
        assert!(ctx.authorize_command("cargo test").is_err());
    }
}
