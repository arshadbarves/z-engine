//! Mutation- and command-gate tests at the tools seam: the same
//! verdicts the pure gate reaches, reached through a real ToolCtx.

use super::*;
use crate::governance::gate::GateFailure;
use crate::governance::{AcceptanceCommand, WorkOrder};
use crate::lsp::SymbolAnswer;
use crate::tools::semantics::StubSemantics;
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

/// A guarded run that has read `lib.rs` and declared an order over it,
/// with `semantics` scripted for whatever the test needs to prove.
fn ready(semantics: StubSemantics) -> (ToolCtx, tempfile::TempDir, tempfile::TempDir) {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("lib.rs"), LIB).unwrap();
    let (ctx, store) = guarded_ctx(repo.path(), Some(semantics));
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
    let (ctx, _store) = guarded_ctx(repo.path(), Some(StubSemantics::resolving(&["parse"])));
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
    let (ctx, _store, repo) = ready(StubSemantics::resolving(&["parse"]));
    ctx.authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
        .await
        .expect("scoped, evidence-backed, localized edit must pass");
}

#[tokio::test]
async fn guarded_runs_refuse_paths_outside_the_declared_scope() {
    let (ctx, _store, repo) = ready(StubSemantics::resolving(&["parse"]));
    std::fs::write(repo.path().join("other.rs"), LIB).unwrap();
    let err = ctx
        .authorize_mutation(&repo.path().join("other.rs"), LIB.as_bytes(), Some((1, 1)))
        .await
        .unwrap_err();
    assert!(matches!(err, GateFailure::OutOfScope { .. }), "{err}");
}

#[tokio::test]
async fn guarded_runs_refuse_bytes_that_no_longer_match_the_read() {
    let (ctx, _store, repo) = ready(StubSemantics::resolving(&["parse"]));
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
    let (ctx, _store, repo) = ready(StubSemantics::unavailable("spawn failed"));
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
    let (ctx, _store) = guarded_ctx(repo.path(), Some(StubSemantics::resolving(&["parse"])));
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

/// The load-bearing case for finding 2: the text really does declare
/// the symbol, but the language server does not place it here. A
/// tree-sitter outline is not evidence, so this must refuse.
#[tokio::test]
async fn a_tree_sitter_match_cannot_stand_in_for_semantic_evidence() {
    let (ctx, _store, repo) = ready(StubSemantics::resolving(&["render"]));
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
async fn an_unindexed_file_blocks_instead_of_passing_on_an_empty_answer() {
    let (ctx, _store, repo) = ready(StubSemantics::answering(SymbolAnswer::Unindexed(
        "the server reported no symbols for this file".into(),
    )));
    let err = ctx
        .authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
        .await
        .unwrap_err();
    assert!(
        matches!(err, GateFailure::SemanticEvidenceUnavailable { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn an_answer_about_another_document_is_never_trusted() {
    let (ctx, _store, repo) = ready(StubSemantics::answering(SymbolAnswer::Mismatched(
        "symbols were reported for file:///elsewhere.rs".into(),
    )));
    let err = ctx
        .authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), Some((2, 2)))
        .await
        .unwrap_err();
    assert!(
        matches!(err, GateFailure::SemanticEvidenceMismatch { .. }),
        "{err}"
    );
}

/// Cheap rules answer first: an out-of-scope path is refused as such
/// even when the semantic provider would also have blocked it.
#[tokio::test]
async fn semantics_are_not_consulted_for_a_change_the_order_already_refuses() {
    let (ctx, _store, repo) = ready(StubSemantics::unavailable("spawn failed"));
    std::fs::write(repo.path().join("other.rs"), LIB).unwrap();
    let err = ctx
        .authorize_mutation(&repo.path().join("other.rs"), LIB.as_bytes(), Some((1, 1)))
        .await
        .unwrap_err();
    assert!(matches!(err, GateFailure::OutOfScope { .. }), "{err}");
}

/// The finding this binding exists for, at the seam a tool uses.
///
/// `TWO_FNS` declares `parse` and `render`; the order names only
/// `parse`. Editing inside `parse` passes, editing `render` — same
/// file, same read evidence, same order — must not.
#[tokio::test]
async fn a_change_to_a_sibling_symbol_in_a_scoped_file_is_refused() {
    const TWO_FNS: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n\npub fn render(n: usize) -> String {\n    n.to_string()\n}\n";
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("lib.rs"), TWO_FNS).unwrap();
    let (ctx, _store) = guarded_ctx(
        repo.path(),
        Some(StubSemantics::placing(&[
            ("parse", (1, 3)),
            ("render", (5, 7)),
        ])),
    );
    let id = read(&ctx, "lib.rs", TWO_FNS.as_bytes());
    ctx.set_work_order(&order(&["lib.rs"], &["parse"], &[&id]))
        .unwrap();

    ctx.authorize_mutation(
        &repo.path().join("lib.rs"),
        TWO_FNS.as_bytes(),
        Some((2, 2)),
    )
    .await
    .expect("a change inside the declared symbol must pass");

    let err = ctx
        .authorize_mutation(
            &repo.path().join("lib.rs"),
            TWO_FNS.as_bytes(),
            Some((6, 6)),
        )
        .await
        .unwrap_err();
    let GateFailure::ChangeOutsideTargetSymbol {
        changed, symbols, ..
    } = &err
    else {
        panic!("editing an undeclared neighbour must be refused: {err}");
    };
    assert_eq!(changed, "line 6");
    assert!(symbols.contains("parse (lines 1-3)"), "{symbols}");
}

/// A whole-file Rust rewrite names no symbol, so a symbol-scoped
/// order cannot authorize it however wide the read evidence is.
#[tokio::test]
async fn a_whole_file_rust_rewrite_is_refused_even_with_whole_file_evidence() {
    let (ctx, _store, repo) = ready(StubSemantics::resolving(&["parse"]));
    let err = ctx
        .authorize_mutation(&repo.path().join("lib.rs"), LIB.as_bytes(), None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, GateFailure::ChangeOutsideTargetSymbol { .. }),
        "{err}"
    );
}

#[test]
fn guarded_runs_allow_provably_read_only_commands_and_refuse_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    let (ctx, _store) = guarded_ctx(tmp.path(), Some(StubSemantics::resolving(&["parse"])));
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
