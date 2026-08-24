//! `glob` — pattern → project paths, gitignore-aware, capped (spec §7).

use async_trait::async_trait;
use globset::Glob;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile};

const DEFAULT_CAP: usize = 500;

#[derive(Debug)]
pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "List files matching a glob pattern (e.g. `**/*.rs`, `src/*.toml`), \
         relative to the project root. Respects .gitignore. Capped at 500."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob relative to project root."},
                "cap": {"type": "integer", "description": "Max results (default 500)."}
            },
            "required": ["pattern"]
        })
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "glob",
            problem: "input must be an object".into(),
        })?;
        let pattern = obj
            .get("pattern")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "glob",
                problem: "`pattern` must be a non-empty string".into(),
            })?;
        let cap = obj
            .get("cap")
            .and_then(Value::as_u64)
            .map(|c| c as usize)
            .unwrap_or(DEFAULT_CAP)
            .clamp(1, 5_000);

        let glob = Glob::new(pattern)
            .map_err(|e| ToolError::InvalidInput {
                tool: "glob",
                problem: format!("bad pattern `{pattern}`: {e}"),
            })?
            .compile_matcher();

        let walker = ignore::WalkBuilder::new(&ctx.project_root)
            .hidden(true)
            .git_ignore(true)
            .require_git(false)
            .build();

        let mut matches: Vec<String> = Vec::new();
        let mut total = 0usize;
        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(&ctx.project_root) else {
                continue;
            };
            if glob.is_match(rel) {
                total += 1;
                if matches.len() < cap {
                    matches.push(rel.to_string_lossy().into_owned());
                }
            }
        }
        matches.sort();

        let body = match matches.len() {
            0 => format!("no files match `{pattern}`"),
            _ => format!(
                "{} file(s) match `{pattern}`{}:\n{}",
                matches.len(),
                if total > matches.len() {
                    format!(" (showing first {cap} of {total})")
                } else {
                    String::new()
                },
                matches.join("\n")
            ),
        };

        let result = truncate_with_tempfile(&body, ctx);
        Ok(ToolOutput::success(
            result,
            format!("glob {pattern}: {} hits", matches.len()),
        ))
    }
}
