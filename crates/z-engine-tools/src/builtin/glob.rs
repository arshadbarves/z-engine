//! `Glob`: files whose path matches a glob, newest first, gitignore-aware.

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::{HostError, glob};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, path_field, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::truncate_output;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        names::GLOB
    }

    fn description(&self) -> String {
        prompts::GLOB.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "pattern": {"type": "string", "description": "The glob pattern, e.g. \"**/*.rs\" or \"src/**/*.{ts,tsx}\"."},
                "path": {"type": "string", "description": "The directory to search in (default: the project root). Omit it rather than passing an empty value."}
            }),
            &["pattern"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action {
        let base = path_field(input, "path", ctx).unwrap_or_else(|| ctx.root.clone());
        // An absolute pattern reads below its literal prefix, not the base.
        let target = str_field(input, "pattern")
            .filter(|pattern| Path::new(pattern.trim()).is_absolute())
            .map_or(base, |pattern| literal_prefix(pattern.trim()));
        Action::Read {
            paths: vec![target],
        }
    }

    fn title(&self, input: &Value, ctx: &ToolCtx) -> String {
        let pattern = str_field(input, "pattern").unwrap_or("");
        match path_field(input, "path", ctx) {
            Some(path) => format!("Glob {pattern} in {}", ctx.display(&path)),
            None => format!("Glob {pattern}"),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let pattern = fields.non_empty("pattern")?.to_string();
        let base = fields.optional_text("path")?.map(|path| ctx.resolve(path));
        let root = ctx.root.clone();
        let limit = ctx.limits.glob_limit;
        let search_base = base.clone();
        let found = tokio::task::spawn_blocking(move || {
            glob(&root, &pattern, search_base.as_deref(), limit)
        })
        .await
        .map_err(|e| ToolError::failed(format!("the file search failed: {e}")))?
        .map_err(|e| match e {
            HostError::NotFound(_) => ToolError::failed(format!(
                "Directory does not exist: {}",
                base.as_deref()
                    .map_or_else(|| ctx.root.display().to_string(), |b| ctx.display(b))
            )),
            other => other.into(),
        })?;
        if found.paths.is_empty() {
            return Ok(ToolOutput::text("No files found", "No files found"));
        }
        let mut text: String = found
            .paths
            .iter()
            .map(|path| format!("{}\n", ctx.display(path)))
            .collect();
        if found.truncated {
            text.push_str(&format!(
                "(Results are truncated to the {limit} most recently modified files. Use a more specific pattern or path.)\n"
            ));
        }
        let count = found.paths.len();
        let summary = if found.truncated {
            format!("Found more than {count} files")
        } else {
            format!(
                "Found {count} {}",
                if count == 1 { "file" } else { "files" }
            )
        };
        Ok(ToolOutput::text(
            truncate_output(ctx, "glob", &text),
            summary,
        ))
    }
}

/// The directories of an absolute pattern before its first glob syntax.
fn literal_prefix(pattern: &str) -> PathBuf {
    let mut prefix = PathBuf::new();
    for component in Path::new(pattern).components() {
        if let Component::Normal(part) = component {
            if part.to_string_lossy().contains(['*', '?', '[', '{']) {
                break;
            }
        }
        prefix.push(component.as_os_str());
    }
    if prefix == Path::new(pattern) {
        prefix.pop();
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_patterns_read_below_their_literal_prefix() {
        assert_eq!(literal_prefix("/etc/**/*.conf"), PathBuf::from("/etc"));
        assert_eq!(literal_prefix("/a/b/file.txt"), PathBuf::from("/a/b"));
        assert_eq!(literal_prefix("/*.rs"), PathBuf::from("/"));
    }
}
