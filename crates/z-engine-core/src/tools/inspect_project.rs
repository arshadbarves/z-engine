use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_project::{DiscoveryError, DiscoveryOptions, discover_with_cancel};

use super::{Tool, ToolCtx, ToolError, ToolOutput};

#[derive(Debug)]
pub struct InspectProjectTool;

#[async_trait]
impl Tool for InspectProjectTool {
    fn name(&self) -> &str {
        "inspect_project"
    }

    fn description(&self) -> &str {
        "Read-only, bounded workspace discovery of language/project markers and existing \
         verification command suggestions with exact working directories. Does not execute \
         commands, probe installed tools, or provide semantic-refactoring or completion evidence."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "max_depth": {"type": "integer", "minimum": 0, "maximum": 16, "default": 8},
                "max_entries": {"type": "integer", "minimum": 1, "maximum": 50000, "default": 20000},
                "max_profiles": {"type": "integer", "minimum": 1, "maximum": 256, "default": 128},
                "max_manifest_bytes": {"type": "integer", "minimum": 1, "maximum": 1048576, "default": 262144},
                "max_total_bytes": {"type": "integer", "minimum": 1, "maximum": 8388608, "default": 4194304}
            },
            "required": []
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let invalid = |problem: String| ToolError::InvalidInput {
            tool: "inspect_project",
            problem,
        };
        if !input.is_object() {
            return Err(invalid("input must be an object".into()));
        }
        let options: DiscoveryOptions =
            serde_json::from_value(input).map_err(|error| invalid(error.to_string()))?;
        options
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        let root = ctx.project_root.clone();
        let cancel = ctx.abort.clone();
        let report =
            tokio::task::spawn_blocking(move || discover_with_cancel(&root, &options, &cancel))
                .await
                .map_err(|error| {
                    ToolError::Failed(format!("project discovery worker failed: {error}"))
                })?
                .map_err(|error| match error {
                    DiscoveryError::InvalidOptions(_) => invalid(error.to_string()),
                    _ => ToolError::Failed(error.to_string()),
                })?;
        let summary = format!(
            "inspect_project: {} profiles, {} languages{}; no commands executed",
            report.profiles.len(),
            report.languages.len(),
            if report.scan.complete {
                ""
            } else {
                " (partial scan)"
            },
        );
        let output = serde_json::to_string(&report)
            .map_err(|error| ToolError::Failed(format!("serialize project report: {error}")))?;
        Ok(if report.has_errors() {
            ToolOutput::failure(output, format!("{summary}; inspect diagnostics"))
        } else {
            ToolOutput::success(output, summary)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::perms::PolicyEngine;

    fn context(directory: &std::path::Path) -> ToolCtx {
        ToolCtx::new(
            directory.into(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            directory.into(),
        )
    }

    #[tokio::test]
    async fn strict_input_and_read_only_contract() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = context(directory.path());
        assert!(InspectProjectTool.concurrency_safe());
        assert!(
            InspectProjectTool
                .approval_preview(&json!({}), &ctx)
                .is_none()
        );
        for input in [
            json!(null),
            json!([]),
            json!({"path": ".."}),
            json!({"max_depth": -1}),
            json!({"max_profiles": 0}),
            json!({"max_entries": 50001}),
            json!({"max_depth": 1.5}),
            json!({"max_depth": null}),
        ] {
            assert!(matches!(
                InspectProjectTool.run(input, &ctx).await,
                Err(ToolError::InvalidInput { .. })
            ));
        }
    }

    #[tokio::test]
    async fn structured_output_and_manifest_errors() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = context(directory.path());
        std::fs::write(directory.path().join("package.json"), "{bad").unwrap();
        let output = InspectProjectTool.run(json!({}), &ctx).await.unwrap();
        assert!(!output.ok);
        let report: Value = serde_json::from_str(&output.result).unwrap();
        assert_eq!(report["execution"], "not_run");
        assert_eq!(
            report["profiles"][0]["diagnostics"][0]["code"],
            "malformed_manifest"
        );
        assert_eq!(
            report["profiles"][0]["capabilities"]["verified_completion"],
            "not_provided"
        );
        assert!(ctx.tracked_paths().is_empty());
    }
}
