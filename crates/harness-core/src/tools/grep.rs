//! `grep` — regex search over the project: ripgrep subprocess fast-path,
//! pure-Rust `ignore` + `regex` fallback (spec §2 allows exactly one
//! optional external binary).

use std::io::BufRead;
use std::sync::atomic::{AtomicU8, Ordering};

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile};

const DEFAULT_CAP: usize = 100;
const MAX_CAP: usize = 1_000;
/// Files larger than this are skipped by the fallback scanner.
const FALLBACK_MAX_FILE_BYTES: u64 = 1_000_000;

/// 0 unknown · 1 present · 2 absent
static RG_AVAILABILITY: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone)]
struct Hit {
    path: String,
    line: usize,
    text: String,
}

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

fn rg_available() -> bool {
    match RG_AVAILABILITY.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let ok = std::process::Command::new("rg")
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            RG_AVAILABILITY.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            ok
        }
    }
}

async fn run_ripgrep(
    ctx: &ToolCtx,
    pattern: &str,
    glob: Option<&str>,
    ignore_case: bool,
    limit: usize,
) -> Result<Vec<Hit>, String> {
    let mut cmd = tokio::process::Command::new("rg");
    cmd.args([
        "--no-heading",
        "--line-number",
        "--color",
        "never",
        "--max-columns",
        "400",
        "--sort",
        "path",
        // project roots may not be inside a git repo, yet .gitignore should
        // still govern what we search (matches the pure-Rust fallback)
        "--no-require-git",
    ]);
    if let Some(g) = glob {
        cmd.arg("-g").arg(g);
    }
    if ignore_case {
        cmd.arg("-i");
    }
    cmd.arg("-e").arg(pattern).arg(".");
    cmd.current_dir(&ctx.project_root);

    let out = cmd.output().await.map_err(|e| format!("spawn rg: {e}"))?;
    if !out.status.success() && out.status.code() != Some(1) {
        // rg: 0 matches, 1 no-matches, ≥2 real errors
        return Err(format!(
            "rg exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(300)
                .collect::<String>()
        ));
    }

    let mut hits = Vec::new();
    for line in out.stdout.lines() {
        let Ok(line) = line else { break };
        // path:line:text — paths from rg contain no colons unless escaped;
        // split into exactly three parts from the right for line:text.
        let mut parts = line.splitn(3, ':');
        let (Some(path), Some(line_no), Some(text)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(line_no) = line_no.parse::<usize>() else {
            continue;
        };
        hits.push(Hit {
            // rg prints `./src/a.rs` when rooted at "."; normalize
            path: path.strip_prefix("./").unwrap_or(path).to_string(),
            line: line_no,
            text: text.to_string(),
        });
        if hits.len() >= limit {
            break;
        }
    }
    Ok(hits)
}

fn run_fallback(
    ctx: &ToolCtx,
    pattern: &str,
    glob: Option<&str>,
    ignore_case: bool,
    limit: usize,
) -> Result<Vec<Hit>, ToolError> {
    let mut builder = regex::RegexBuilder::new(pattern);
    builder.case_insensitive(ignore_case);
    let re = builder.build().map_err(|e| ToolError::InvalidInput {
        tool: "grep",
        problem: format!("bad pattern `{pattern}`: {e}"),
    })?;
    let gs = glob.map(|g| {
        GlobSetHolder(
            GlobSetBuilder::new()
                .add(Glob::new(g).expect("glob compiled earlier? validated here"))
                .build()
                .expect("valid glob"),
        )
    });

    let walker = ignore::WalkBuilder::new(&ctx.project_root)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .build();

    let mut hits = Vec::new();
    'files: for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() > FALLBACK_MAX_FILE_BYTES {
            continue;
        }
        let rel = match entry.path().strip_prefix(&ctx.project_root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if let Some(holder) = &gs {
            if !holder.0.is_match(rel) {
                continue;
            }
        }
        let Ok(f) = std::fs::File::open(entry.path()) else {
            continue;
        };
        let reader = std::io::BufReader::new(f);
        for (idx, line) in reader.lines().enumerate() {
            let Ok(line) = line else { break }; // non-utf8 ⇒ skip rest of file
            if re.is_match(&line) {
                hits.push(Hit {
                    path: rel.to_string_lossy().into_owned(),
                    line: idx + 1,
                    text: line.chars().take(400).collect(),
                });
                if hits.len() >= limit {
                    break 'files;
                }
            }
        }
    }
    Ok(hits)
}

// globset imports tucked at bottom to keep the top clean
use globset::{Glob, GlobSet, GlobSetBuilder};
struct GlobSetHolder(GlobSet);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    fn ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    fn fixture(root: &Path) {
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        std::fs::write(root.join("src/a.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
        std::fs::write(root.join("src/deep/b.rs"), "let alpha_count = 1;\n").unwrap();
        std::fs::write(root.join("notes.md"), "# alpha notes\n").unwrap();
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir_all(root.join("ignored")).unwrap();
        std::fs::write(root.join("ignored/secret.rs"), "alpha here\n").unwrap();
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
    async fn fallback_engine_agrees_with_ripgrep_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let ctx = ctx_in(tmp.path());

        let fb = run_fallback(&ctx, "beta", None, false, 50).unwrap();
        assert_eq!(fb.len(), 1);
        assert_eq!(fb[0].path, "src/a.rs");
        assert_eq!(fb[0].line, 2);

        if rg_available() {
            let rg = run_ripgrep(&ctx, "beta", None, false, 50).await.unwrap();
            assert_eq!(rg.len(), 1);
            assert_eq!(rg[0].path, fb[0].path);
            assert_eq!(rg[0].line, fb[0].line);
        }
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
