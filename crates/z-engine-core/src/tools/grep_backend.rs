//! Search backends for the `grep` tool: ripgrep subprocess fast-path and
//! pure-Rust `ignore` + `regex` fallback (spec §2 allows exactly one
//! optional external binary).

use std::io::BufRead;
#[cfg(test)]
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

use globset::{Glob, GlobSet, GlobSetBuilder};

use super::{ToolCtx, ToolError};

/// Files larger than this are skipped by the fallback scanner.
const FALLBACK_MAX_FILE_BYTES: u64 = 1_000_000;

/// 0 unknown · 1 present · 2 absent
static RG_AVAILABILITY: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone)]
pub(super) struct Hit {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) text: String,
}

pub(super) fn rg_available() -> bool {
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

pub(super) async fn run_ripgrep(
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

pub(super) fn run_fallback(
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
    // Model-supplied: validate instead of panicking (v1.0 error audit).
    let gs = match glob {
        Some(g) => {
            let glob = Glob::new(g).map_err(|e| ToolError::InvalidInput {
                tool: "grep",
                problem: format!("bad glob `{g}`: {e}"),
            })?;
            Some(GlobSetHolder(
                GlobSetBuilder::new()
                    .add(glob)
                    .build()
                    .map_err(|e| ToolError::InvalidInput {
                        tool: "grep",
                        problem: format!("bad glob set: {e}"),
                    })?,
            ))
        }
        None => None,
    };

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

struct GlobSetHolder(GlobSet);

/// Shared test fixture: a tiny project tree exercising gitignore handling.
#[cfg(test)]
pub(super) fn fixture(root: &Path) {
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::write(root.join("src/a.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
    std::fs::write(root.join("src/deep/b.rs"), "let alpha_count = 1;\n").unwrap();
    std::fs::write(root.join("notes.md"), "# alpha notes\n").unwrap();
    std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    std::fs::create_dir_all(root.join("ignored")).unwrap();
    std::fs::write(root.join("ignored/secret.rs"), "alpha here\n").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn backend_ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    #[tokio::test]
    async fn fallback_engine_agrees_with_ripgrep_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let ctx = backend_ctx_in(tmp.path());

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
}
