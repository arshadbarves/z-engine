//! `edit_file` — surgical string replacement tool. The matching strategy is
//! the spec §7 ladder implemented in [`edit_ladder`] (exact → hint → fuzzy).
//!
//! Read-before-edit is enforced via [`FileStateTracker`], and stale reads
//! (file changed since) force a re-read.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;

use super::edit_ladder::{Replacement, apply_ladder};
use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile, unified_diff};
use crate::governance::changed_line_range;

/// How far (in lines) a hint may be from a match to count as "nearest".
pub(crate) const PREVIEW_DIFF_CHARS: usize = 1_600;

#[derive(Debug)]
pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Replace an exact snippet in a file. If the snippet matches several \
         times pass `line_hint` (1-based line near the intended spot); if it \
         doesn't match exactly, close variants are found fuzzily. The file \
         must have been read first."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path relative to project root (or absolute)."},
                "old_string": {"type": "string", "description": "Exact text to replace (include surrounding context to make it unique)."},
                "new_string": {"type": "string", "description": "Replacement text."},
                "line_hint": {"type": "integer", "description": "Optional 1-based line number near the intended match."}
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    fn approval_preview(&self, input: &Value, ctx: &ToolCtx) -> Option<String> {
        let path = input.get("path")?.as_str()?;
        let resolved = ctx.resolve(Path::new(path));
        let disp = super::write_file::rel(&resolved, &ctx.project_root);
        let old = String::from_utf8_lossy(&std::fs::read(&resolved).ok()?).into_owned();
        let old_snip = input.get("old_string")?.as_str()?;
        let new_snip = input.get("new_string")?.as_str()?;
        // Preview against the unique-exact outcome; ladder may differ at run
        // time but this is what the human is approving conceptually.
        let new_full = old.replacen(old_snip, new_snip, 1);
        let d = unified_diff(&old, &new_full, &disp);
        let mut out: String = d.chars().take(PREVIEW_DIFF_CHARS).collect();
        if out.chars().count() == PREVIEW_DIFF_CHARS {
            out.push_str("\n… (diff truncated)");
        }
        Some(out)
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "edit_file",
            problem: "input must be an object".into(),
        })?;
        let raw_path =
            obj.get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidInput {
                    tool: "edit_file",
                    problem: "`path` must be a string".into(),
                })?;
        let old_s = obj
            .get("old_string")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "edit_file",
                problem: "`old_string` must be a non-empty string".into(),
            })?;
        let new_s = obj
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "edit_file",
                problem: "`new_string` must be a string".into(),
            })?;
        let line_hint = obj
            .get("line_hint")
            .and_then(Value::as_u64)
            .map(|v| v as usize);

        let resolved = ctx.resolve(Path::new(raw_path));
        let disp = super::write_file::rel(&resolved, &ctx.project_root);
        if !resolved.exists() {
            return Ok(ToolOutput::failure(
                format!("ERROR: {disp} does not exist — use write_file to create files"),
                format!("edit_file: missing {disp}"),
            ));
        }
        ctx.require_read_for_mutation("edit_file", &resolved, true)?;
        // Rewind support: stash the pre-edit image before touching disk.
        ctx.checkpoint_before_mutation(&resolved);

        let bytes = tokio::fs::read(&resolved)
            .await
            .map_err(|e| ToolError::Failed(format!("read {}: {e}", resolved.display())))?;
        // Strict UTF-8: lossy conversion would silently corrupt binary
        // files by replacing invalid bytes with U+FFFD on write-back.
        let current = String::from_utf8(bytes).map_err(|_| {
            ToolError::Failed(format!(
                "{disp} is not valid UTF-8 text; refusing to edit it"
            ))
        })?;

        let rep: Replacement = apply_ladder(&current, old_s, new_s, line_hint).map_err(|msg| {
            // Model-visible guidance so it can adjust (never crash).
            ToolError::Failed(msg)
        })?;

        // Guarded runs authorize the *localized* patch against the work
        // order and this run's evidence before anything reaches disk.
        // `current` is the snapshot the ladder matched against, so the
        // gate judges exactly the bytes about to be replaced.
        ctx.authorize_mutation(
            &resolved,
            current.as_bytes(),
            changed_line_range(&current, &rep.new_content),
        )
        .await?;

        super::atomic_write(&resolved, rep.new_content.as_bytes())
            .await
            .map_err(|e| ToolError::Failed(format!("write {disp}: {e}")))?;
        ctx.note_read(&resolved);

        let diff = unified_diff(&current, &rep.new_content, &disp);
        let body = format!("{disp}: edited (match: {})\n{diff}", rep.rung);
        let result = truncate_with_tempfile(&body, ctx);
        // Reviewer journal entry (spec section 9 v0.9).
        if let Ok(mut j) = ctx.edit_journal.lock() {
            j.push(result.clone());
        }
        tracing::debug!(path = %disp, rung = rep.rung, "edit_file done");
        Ok(ToolOutput::success(
            result,
            format!("edited {disp} ({})", rep.rung),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    #[tokio::test]
    async fn refuses_without_prior_read_and_after_staleness() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("e.txt");
        std::fs::write(&p, "hello world\n").unwrap();

        // no read yet
        let err = EditFileTool
            .run(
                json!({"path": "e.txt", "old_string": "world", "new_string": "there"}),
                &ctx_in(tmp.path()),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("without reading"));

        // read → stale via external change
        let ctx = ctx_in(tmp.path());
        ctx.note_read(&p);
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&p, "changed externally\n").unwrap();
        let err = EditFileTool
            .run(
                json!({"path": "e.txt", "old_string": "x", "new_string": "y"}),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("changed on disk"), "{err}");

        // fresh read → success
        ctx.note_read(&p);
        let out = EditFileTool
            .run(
                json!({"path": "e.txt", "old_string": "external", "new_string": "internal"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("-changed externally"));
        assert!(out.result.contains("+changed internally"));
    }

    #[tokio::test]
    async fn end_to_end_edit_updates_file_and_reports_rung() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("m.txt");
        std::fs::write(&p, "one\ntwo\nthree\nfour\nfive\n").unwrap();
        let ctx = ctx_in(tmp.path());
        ctx.note_read(&p);

        let out = EditFileTool
            .run(
                json!({"path": "m.txt", "old_string": "two\nthree\nfour", "new_string": "TWO\nTHREE"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.summary.contains("exact"));
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "one\nTWO\nTHREE\nfive\n"
        );
    }

    // ---- guarded mode (Task 5): authorization runs before any write ----

    const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";

    /// A guarded context that has read `lib.rs` and (optionally) declared
    /// an order over it, mirroring what `read_file` + `set_work_order` do.
    fn guarded_lib(repo: &Path, with_order: bool) -> (ToolCtx, tempfile::TempDir) {
        std::fs::write(repo.join("lib.rs"), LIB).unwrap();
        let (ctx, store) = crate::tools::test_support::guarded_ctx(
            repo,
            Some(crate::tools::semantics::StubSemantics::resolving(&[
                "parse",
            ])),
        );
        let resolved = ctx.resolve(Path::new("lib.rs"));
        ctx.note_read(&resolved);
        let id = ctx
            .record_read_evidence(&resolved, None, LIB.as_bytes(), LIB.as_bytes())
            .unwrap()
            .unwrap();
        if with_order {
            ctx.set_work_order(&crate::governance::WorkOrder {
                id: "wo-1".into(),
                goal: "make parse fallible".into(),
                writable_paths: vec![std::path::PathBuf::from("lib.rs")],
                target_symbols: vec!["parse".into()],
                evidence_ids: vec![id],
                acceptance_commands: Vec::new(),
            })
            .unwrap();
        }
        (ctx, store)
    }

    #[tokio::test]
    async fn guarded_edits_without_a_work_order_never_touch_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _store) = guarded_lib(tmp.path(), false);

        let err = EditFileTool
            .run(
                json!({"path": "lib.rs", "old_string": "s.len()", "new_string": "s.len() + 1"}),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("set_work_order"), "{err}");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("lib.rs")).unwrap(),
            LIB
        );
    }

    #[tokio::test]
    async fn guarded_edits_inside_the_order_still_apply() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _store) = guarded_lib(tmp.path(), true);

        let out = EditFileTool
            .run(
                json!({"path": "lib.rs", "old_string": "s.len()", "new_string": "s.len() + 1"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.ok, "{}", out.result);
        assert!(
            std::fs::read_to_string(tmp.path().join("lib.rs"))
                .unwrap()
                .contains("s.len() + 1")
        );
    }

    #[tokio::test]
    async fn guarded_edits_outside_the_order_scope_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _store) = guarded_lib(tmp.path(), true);
        let other = tmp.path().join("other.rs");
        std::fs::write(&other, LIB).unwrap();
        ctx.note_read(&other);

        let err = EditFileTool
            .run(
                json!({"path": "other.rs", "old_string": "s.len()", "new_string": "0"}),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("not in the active work order"),
            "{err}"
        );
        assert_eq!(std::fs::read_to_string(&other).unwrap(), LIB);
    }
}
