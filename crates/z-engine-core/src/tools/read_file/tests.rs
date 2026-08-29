use super::*;
use crate::evidence::{BlobStore, EvidenceLedger, FsBlobStore};
use crate::perms::PolicyEngine;
use crate::tools::EvidenceStore;
use std::sync::{Arc, Mutex};

fn ctx_in(dir: &std::path::Path) -> ToolCtx {
    ToolCtx::new(
        dir.to_path_buf(),
        Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
        tempfile::tempdir().unwrap().keep(),
    )
}

/// A `ToolCtx` with a fresh, temporary evidence recorder attached. The
/// returned `TempDir` and blob store must both stay alive/bound for the
/// caller's whole test: dropping the `TempDir` early deletes the ledger
/// and blob files out from under the store.
fn ctx_with_evidence(
    dir: &std::path::Path,
) -> (ToolCtx, Arc<dyn BlobStore + Send + Sync>, tempfile::TempDir) {
    let evidence_dir = tempfile::tempdir().unwrap();
    let ledger = Arc::new(EvidenceLedger::open(evidence_dir.path()).unwrap());
    let blobs: Arc<dyn BlobStore + Send + Sync> =
        Arc::new(FsBlobStore::new(evidence_dir.path().join("blobs")).unwrap());
    let ctx = ctx_in(dir).with_evidence(Arc::new(EvidenceStore::new(ledger, Arc::clone(&blobs))));
    (ctx, blobs, evidence_dir)
}

#[tokio::test]
async fn reads_with_line_numbers() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("a.txt");
    std::fs::write(&p, "alpha\nbeta\ngamma\n").unwrap();
    let out = ReadFileTool
        .run(json!({"path": "a.txt"}), &ctx_in(tmp.path()))
        .await
        .unwrap();
    assert!(out.ok);
    assert!(out.result.contains("   1 │ alpha"));
    assert!(out.result.contains("   3 │ gamma"));
    assert!(out.summary.contains("lines 1–3"));
}

#[tokio::test]
async fn offset_and_limit_window() {
    let tmp = tempfile::tempdir().unwrap();
    let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    std::fs::write(tmp.path().join("b.txt"), content).unwrap();
    let out = ReadFileTool
        .run(
            json!({"path": "b.txt", "offset": 4, "limit": 3}),
            &ctx_in(tmp.path()),
        )
        .await
        .unwrap();
    assert!(out.result.contains("   4 │ line4"));
    assert!(out.result.contains("   6 │ line6"));
    assert!(!out.result.contains("line7"));
    assert!(out.result.contains("more lines follow"));
}

#[tokio::test]
async fn offset_past_end_is_a_model_visible_error() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("c.txt"), "one\n").unwrap();
    let out = ReadFileTool
        .run(json!({"path": "c.txt", "offset": 99}), &ctx_in(tmp.path()))
        .await
        .unwrap();
    assert!(!out.ok);
    assert!(out.result.contains("past end of file"));
}

#[tokio::test]
async fn missing_file_reports_error_not_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let out = ReadFileTool
        .run(json!({"path": "nope/missing.txt"}), &ctx_in(tmp.path()))
        .await
        .unwrap();
    assert!(!out.ok);
    assert!(out.result.contains("file not found"));
}

#[tokio::test]
async fn binary_files_are_not_dumped() {
    let tmp = tempfile::tempdir().unwrap();
    let bytes: Vec<u8> = [0x89u8, b'P', b'N', b'G', 0u8, 1, 2, 3].to_vec();
    std::fs::write(tmp.path().join("img.bin"), &bytes).unwrap();
    let out = ReadFileTool
        .run(json!({"path": "img.bin"}), &ctx_in(tmp.path()))
        .await
        .unwrap();
    assert!(out.ok);
    assert!(out.result.contains("[binary file;"));
    assert!(!out.result.contains("\u{0}"));
}

#[tokio::test]
async fn empty_file_and_absolute_paths() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("e.txt"), "").unwrap();
    let ctx = ctx_in(tmp.path());
    let out = ReadFileTool
        .run(json!({"path": tmp.path().join("e.txt")}), &ctx)
        .await
        .unwrap();
    assert!(out.result.contains("[empty file]"));

    let err = ReadFileTool.run(json!({}), &ctx).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }));
}

#[tokio::test]
async fn read_records_fresh_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let content: String = (1..=25).map(|i| format!("line{i}\n")).collect();
    std::fs::write(tmp.path().join("lib.rs"), &content).unwrap();
    let (ctx, _blobs, _evidence_dir) = ctx_with_evidence(tmp.path());

    let out = ReadFileTool
        .run(json!({"path": "lib.rs", "offset": 1, "limit": 20}), &ctx)
        .await
        .unwrap();
    assert!(out.ok);
    assert!(out.result.contains("[evidence:"));
    assert_eq!(out.evidence_ids.len(), 1);

    let record = ctx
        .fresh_read_evidence(Path::new("lib.rs"))
        .expect("fresh evidence for a just-read file");
    assert_eq!(record.id, out.evidence_ids[0]);
    assert_eq!(record.path, "lib.rs");
    assert_eq!(record.method, "read_file");
    assert_eq!(record.revision, "working-tree");
}

#[tokio::test]
async fn empty_file_reads_record_evidence_and_note_read() {
    // Regression test: successful empty-file reads used to return
    // before either `note_read` (the read-before-edit tracker) or
    // evidence capture ran, so an empty file could never be edited
    // right after being read, and never produced grounding evidence.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("e.txt"), "").unwrap();
    let (ctx, blobs, _evidence_dir) = ctx_with_evidence(tmp.path());

    let out = ReadFileTool
        .run(json!({"path": "e.txt"}), &ctx)
        .await
        .unwrap();
    assert!(out.ok);
    // Output semantics for an empty file are preserved...
    assert!(out.result.contains("[empty file]"));
    // ...with an evidence marker appended, same as non-empty reads.
    assert!(out.result.contains("[evidence:"));
    assert_eq!(out.evidence_ids.len(), 1);

    let record = ctx
        .fresh_read_evidence(Path::new("e.txt"))
        .expect("empty-file read must still produce fresh evidence");
    assert_eq!(record.id, out.evidence_ids[0]);
    assert_eq!(record.line_range, None);
    let range_bytes = blobs.get(&record.blob).unwrap();
    assert!(range_bytes.is_empty());

    // note_read must have fired too, so an immediate edit isn't
    // wrongly refused as "never read".
    assert!(ctx.tracked_paths().contains(&tmp.path().join("e.txt")));
}

#[tokio::test]
async fn limited_read_authorizes_only_its_recorded_range() {
    let tmp = tempfile::tempdir().unwrap();
    let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    std::fs::write(tmp.path().join("b.txt"), &content).unwrap();
    let (ctx, blobs, _evidence_dir) = ctx_with_evidence(tmp.path());

    ReadFileTool
        .run(json!({"path": "b.txt", "offset": 4, "limit": 3}), &ctx)
        .await
        .unwrap();

    let record = ctx.fresh_read_evidence(Path::new("b.txt")).unwrap();
    assert_eq!(record.line_range, Some((4, 6)));
    let range_bytes = blobs.get(&record.blob).unwrap();
    assert_eq!(
        String::from_utf8(range_bytes).unwrap(),
        "line4\nline5\nline6"
    );
}

#[tokio::test]
async fn external_change_invalidates_evidence_freshness() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("s.txt"), "v1\n").unwrap();
    let (ctx, _blobs, _evidence_dir) = ctx_with_evidence(tmp.path());

    ReadFileTool
        .run(json!({"path": "s.txt"}), &ctx)
        .await
        .unwrap();
    assert!(ctx.fresh_read_evidence(Path::new("s.txt")).is_some());

    // External modification must invalidate the recorded evidence —
    // otherwise a stale read could keep authorizing edits.
    std::fs::write(tmp.path().join("s.txt"), "v2 changed on disk\n").unwrap();
    assert!(ctx.fresh_read_evidence(Path::new("s.txt")).is_none());
}

#[tokio::test]
async fn evidence_is_never_created_for_binary_failed_or_unattached_reads() {
    let tmp = tempfile::tempdir().unwrap();
    let bytes: Vec<u8> = [0x89u8, b'P', b'N', b'G', 0u8, 1, 2, 3].to_vec();
    std::fs::write(tmp.path().join("img.bin"), &bytes).unwrap();
    std::fs::write(tmp.path().join("n.txt"), "alpha\n").unwrap();
    let (ctx, _blobs, _evidence_dir) = ctx_with_evidence(tmp.path());

    let binary = ReadFileTool
        .run(json!({"path": "img.bin"}), &ctx)
        .await
        .unwrap();
    assert!(binary.ok && binary.evidence_ids.is_empty());
    assert!(ctx.fresh_read_evidence(Path::new("img.bin")).is_none());

    let missing = ReadFileTool
        .run(json!({"path": "missing.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!missing.ok && missing.evidence_ids.is_empty());
    assert!(ctx.fresh_read_evidence(Path::new("missing.txt")).is_none());

    // No recorder attached at all ⇒ existing (pre-evidence) behavior.
    let unattached = ctx_in(tmp.path());
    let out = ReadFileTool
        .run(json!({"path": "n.txt"}), &unattached)
        .await
        .unwrap();
    assert!(out.ok && !out.result.contains("[evidence:") && out.evidence_ids.is_empty());
    assert!(unattached.fresh_read_evidence(Path::new("n.txt")).is_none());
}
