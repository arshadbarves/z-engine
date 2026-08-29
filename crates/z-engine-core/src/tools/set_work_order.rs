//! `set_work_order` — the guarded-mode scope declaration (Task 4).
//!
//! The model states its goal, the paths it needs to write, the symbols it
//! expects to touch, the evidence ids backing those paths, and how the
//! change will be proven. The harness admits the order only when every
//! writable path is covered by cited, still-fresh read evidence from this
//! run; the accepted order is then pinned into following prompts.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput};
use crate::evidence::BlobHandle;
use crate::governance::{AcceptanceCommand, WorkOrder};

#[derive(Debug)]
pub struct SetWorkOrderTool;

#[derive(Debug, Deserialize)]
struct Input {
    #[serde(default)]
    id: Option<String>,
    goal: String,
    writable_paths: Vec<PathBuf>,
    #[serde(default)]
    target_symbols: Vec<String>,
    #[serde(default)]
    evidence_ids: Vec<String>,
    #[serde(default)]
    acceptance_commands: Vec<AcceptanceCommand>,
}

impl Input {
    fn into_order(self) -> WorkOrder {
        let id = self.id.unwrap_or_else(|| default_id(&self.goal));
        WorkOrder {
            id,
            goal: self.goal,
            writable_paths: self.writable_paths,
            target_symbols: self.target_symbols,
            evidence_ids: self.evidence_ids,
            acceptance_commands: self.acceptance_commands,
        }
    }
}

/// Stable id derived from the goal, so replaying the same declaration
/// yields the same order id (reuses the evidence module's hashing).
fn default_id(goal: &str) -> String {
    format!("wo-{}", &BlobHandle::of(goal.as_bytes()).to_string()[..8])
}

#[async_trait]
impl Tool for SetWorkOrderTool {
    fn name(&self) -> &str {
        "set_work_order"
    }

    fn description(&self) -> &str {
        "Declare the scope of the change you are about to make: the goal, \
         the paths you may write, the symbols you expect to touch, the \
         `[evidence: …]` ids from your reads of those paths, and the \
         commands that will prove completion. Read every writable path \
         first — an order whose paths lack fresh evidence is refused. \
         Setting a new order replaces the previous one."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {"type": "string",
                    "description": "Optional stable id; defaults to a digest of the goal."},
                "goal": {"type": "string",
                    "description": "One line stating what the change must achieve."},
                "writable_paths": {"type": "array", "items": {"type": "string"},
                    "description": "Paths this order may modify; each needs fresh read evidence."},
                "target_symbols": {"type": "array", "items": {"type": "string"},
                    "description": "Symbols the change is expected to touch."},
                "evidence_ids": {"type": "array", "items": {"type": "string"},
                    "description": "Evidence ids from read_file output backing the writable paths."},
                "acceptance_commands": {"type": "array",
                    "items": {"type": "object", "properties": {
                        "command": {"type": "string"},
                        "description": {"type": "string"}
                    }, "required": ["command", "description"]},
                    "description": "Commands that must pass before the order is complete."}
            },
            "required": ["goal", "writable_paths"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let parsed: Input = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
            tool: "set_work_order",
            problem: e.to_string(),
        })?;
        let order = parsed.into_order();
        let active = ctx
            .set_work_order(&order)
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let summary = format!(
            "work order {} scoped to {} path(s)",
            active.order.id,
            active.order.writable_paths.len()
        );
        Ok(ToolOutput::success(active.digest(), summary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{BlobStore, EvidenceLedger, FsBlobStore};
    use crate::governance::WorkOrderStore;
    use crate::perms::PolicyEngine;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    fn guarded_ctx(root: &Path) -> (ToolCtx, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Arc::new(EvidenceLedger::open(dir.path()).unwrap());
        let blobs: Arc<dyn BlobStore + Send + Sync> =
            Arc::new(FsBlobStore::new(dir.path().join("blobs")).unwrap());
        let ctx = ToolCtx::new(
            root.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
        .with_evidence(Arc::new(super::super::EvidenceStore::new(ledger, blobs)))
        .with_work_orders(Arc::new(WorkOrderStore::new()));
        (ctx, dir)
    }

    #[tokio::test]
    async fn accepted_order_is_stored_and_echoed_as_a_digest() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"pub fn parse() {}\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path());
        let id = ctx
            .record_read_evidence(
                &ctx.resolve(Path::new("f.rs")),
                None,
                b"pub fn parse() {}\n",
                b"pub fn parse() {}\n",
            )
            .unwrap()
            .unwrap();

        let out = SetWorkOrderTool
            .run(
                json!({
                    "goal": "make parse fallible",
                    "writable_paths": ["./f.rs"],
                    "target_symbols": ["parse"],
                    "evidence_ids": [id],
                    "acceptance_commands": [{"command": "cargo test", "description": "unit tests"}]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(out.ok);
        assert!(out.result.starts_with("# Active work order (guarded)"));
        assert!(out.result.contains("- f.rs [evidence"));
        assert!(out.summary.contains("scoped to 1 path"));
        let active = ctx.active_work_order().unwrap();
        assert_eq!(active.order.id, default_id("make parse fallible"));
        assert_eq!(active.order.writable_paths, [PathBuf::from("f.rs")]);
    }

    #[tokio::test]
    async fn order_without_fresh_evidence_is_refused_with_model_facing_text() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("f.rs"), b"pub fn parse() {}\n").unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path());

        let err = SetWorkOrderTool
            .run(
                json!({"goal": "edit blind", "writable_paths": ["f.rs"]}),
                &ctx,
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("no fresh evidence"),
            "unhelpful refusal: {err}"
        );
        assert!(ctx.active_work_order().is_none());
    }

    #[tokio::test]
    async fn malformed_input_is_invalid_not_a_silent_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, _dir) = guarded_ctx(tmp.path());
        let err = SetWorkOrderTool
            .run(json!({"goal": "no paths field"}), &ctx)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput { .. }));
    }
}
