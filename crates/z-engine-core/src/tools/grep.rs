//! `grep` — regex search over the project. The [`GrepTool`] validates input
//! and formats results; the engines themselves live in [`grep_backend`].

use async_trait::async_trait;
use serde_json::{Value, json};

use super::grep_backend::{Hit, rg_available, run_fallback, run_ripgrep};
use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile};

const DEFAULT_CAP: usize = 100;
const MAX_CAP: usize = 1_000;

#[derive(Debug)]
pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents with a regex across the project. Optional glob \
         filter (`*.rs`), case-insensitive flag. Output `path:line: text`, \
         capped (default 100 matches). Uses ripgrep when available."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Rust-regex pattern."},
                "glob": {"type": "string", "description": "Only search files matching this glob (e.g. \"*.rs\")."},
                "ignore_case": {"type": "boolean", "description": "Case-insensitive match."},
                "cap": {"type": "integer", "description": "Max matches returned (default 100)."}
            },
            "required": ["pattern"]
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "grep",
            problem: "input must be an object".into(),
        })?;
        let pattern =
            obj.get("pattern")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidInput {
                    tool: "grep",
                    problem: "`pattern` must be a string".into(),
                })?;
        if pattern.trim().is_empty() {
            return Err(ToolError::InvalidInput {
                tool: "grep",
                problem: "`pattern` must not be empty".into(),
            });
        }
        let glob = obj.get("glob").and_then(Value::as_str).map(str::to_string);
        let ignore_case = obj
            .get("ignore_case")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let cap = obj
            .get("cap")
            .and_then(Value::as_u64)
            .map(|c| c as usize)
            .unwrap_or(DEFAULT_CAP)
            .clamp(1, MAX_CAP);

        // Validate regex up front so both engines share semantics.
        let mut builder = regex::RegexBuilder::new(pattern);
        builder.case_insensitive(ignore_case);
        builder.build().map_err(|e| ToolError::InvalidInput {
            tool: "grep",
            problem: format!("bad pattern `{pattern}`: {e}"),
        })?;

        let mut engine = "fallback";
        let hits = if rg_available() {
            match run_ripgrep(ctx, pattern, glob.as_deref(), ignore_case, cap + 1).await {
                Ok(h) => {
                    engine = "ripgrep";
                    h
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ripgrep failed; falling back");
                    run_fallback(ctx, pattern, glob.as_deref(), ignore_case, cap + 1)?
                }
            }
        } else {
            run_fallback(ctx, pattern, glob.as_deref(), ignore_case, cap + 1)?
        };

        let total = hits.len();
        let shown: Vec<Hit> = hits.iter().take(cap).cloned().collect();
        let body = if shown.is_empty() {
            format!("no matches for `{pattern}`")
        } else {
            let mut b = format!(
                "{} match(es) for `{pattern}`{}\n",
                shown.len(),
                if total > cap {
                    format!(" (capped from {total})")
                } else {
                    String::new()
                }
            );
            for h in &shown {
                b.push_str(&format!(
                    "{}:{}: {}\n",
                    h.path,
                    h.line,
                    h.text.replace('\n', " ").replace('\r', "")
                ));
            }
            b
        };

        let result = truncate_with_tempfile(&body, ctx);
        tracing::debug!(engine, hits = shown.len(), "grep done");
        Ok(ToolOutput::success(
            result,
            format!("grep `{pattern}`: {} hit(s)", shown.len()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use super::super::grep_backend::fixture;

    fn ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    #[tokio::test]
    async fn finds_matches_respecting_gitignore_and_cap() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let ctx = ctx_in(tmp.path());

        let out = GrepTool
            .run(json!({"pattern": "alpha"}), &ctx)
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("src/a.rs:1:"), "{}", out.result);
        assert!(out.result.contains("notes.md:1:"));
        assert!(
            !out.result.contains("ignored/secret.rs"),
            "gitignore respected"
        );
        assert_eq!(3, out.result.lines().count() - 1); // header + 3 hits
    }

    #[tokio::test]
    async fn glob_filter_and_case_insensitivity() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let ctx = ctx_in(tmp.path());

        let rs_only = GrepTool
            .run(json!({"pattern": "alpha", "glob": "*.rs"}), &ctx)
            .await
            .unwrap();
        assert!(rs_only.result.contains("src/a.rs"));
        assert!(!rs_only.result.contains("notes.md"));

        let ci = GrepTool
            .run(json!({"pattern": "ALPHA NOTES", "ignore_case": true}), &ctx)
            .await
            .unwrap();
        assert!(ci.result.contains("notes.md:1:"), "{}", ci.result);

        let cs = GrepTool
            .run(json!({"pattern": "ALPHA"}), &ctx)
            .await
            .unwrap();
        assert!(cs.result.starts_with("no matches"));
    }

    #[tokio::test]
    async fn bad_regex_is_invalid_input_not_crash() {
        let tmp = tempfile::tempdir().unwrap();
        let err = GrepTool
            .run(json!({"pattern": "("}), &ctx_in(tmp.path()))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("bad pattern"));
    }

    #[tokio::test]
    async fn cap_limits_output() {
        let tmp = tempfile::tempdir().unwrap();
        let big: String = (1..=500).map(|i| format!("needle{i}\n")).collect();
        std::fs::write(tmp.path().join("big.txt"), big).unwrap();
        let out = GrepTool
            .run(json!({"pattern": "needle", "cap": 10}), &ctx_in(tmp.path()))
            .await
            .unwrap();
        assert!(out.result.contains("(capped from"), "{}", out.result);
        assert_eq!(out.result.lines().count() - 1, 10);
    }
}
