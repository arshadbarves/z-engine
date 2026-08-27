use crate::state::{GuiState, load_workspaces, save_workspaces};
use std::path::{Path, PathBuf};

// ---- diff review (changed files vs HEAD) ------------------------------------

#[derive(serde::Serialize)]
pub(crate) struct ChangedFile {
    path: String,
    status: String,
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Working-tree changes (vs HEAD) for the review panel.
#[tauri::command]
pub(crate) fn list_changed_files(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<ChangedFile>, String> {
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    let porcelain = git(&root, &["status", "--porcelain=v1", "-z"])?;
    let mut out = Vec::new();
    // -z output is NUL-separated: XY<space>path\0[orig\0]
    let mut iter = porcelain.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = iter.next() {
        let mut chars = entry.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let rest = chars.as_str();
        let status = if x != ' ' { x } else { y };
        let path = rest.trim_start().to_string();
        // Renames carry "new\0old\0"; keep the new side only.
        if x == 'R' || y == 'R' {
            iter.next();
        }
        let status = match status {
            '?' => "untracked",
            'A' => "added",
            'D' => "deleted",
            'M' | 'C' => "modified",
            'R' => "renamed",
            other => {
                let _ = other;
                "modified"
            }
        };
        out.push(ChangedFile {
            path,
            status: status.to_string(),
        });
    }
    Ok(out)
}

/// Unified diff of one file against HEAD (full content for untracked).
#[tauri::command]
pub(crate) fn diff_for_file(
    path: String,
    state: tauri::State<'_, GuiState>,
) -> Result<String, String> {
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    // The path is workspace-relative; canonicalize and require it to stay
    // inside the workspace (`..`, absolute paths and symlink escapes all
    // resolve to something that must still be under root).
    if Path::new(&path).is_absolute() {
        return Err("path must be relative to the workspace".into());
    }
    let resolved = std::fs::canonicalize(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("path escapes the workspace: {path}"));
    }
    let tracked = git(&root, &["ls-files", "--error-unmatch", &path]).is_ok();
    if tracked {
        git(&root, &["diff", "HEAD", "--", &path])
    } else {
        let content = std::fs::read_to_string(&resolved).unwrap_or_default();
        let mut out = format!("--- /dev/null\n+++ b/{path}\n");
        for line in content.lines() {
            out.push_str(&format!("+{line}\n"));
        }
        Ok(out)
    }
}

// ---- git worktrees ------------------------------------------------------------

/// Create a linked worktree under `.z-engine/worktrees/<name>` on its own
/// branch, keep it out of `git status`, and register it as a workspace.
#[tauri::command]
pub(crate) fn create_worktree(
    name: String,
    state: tauri::State<'_, GuiState>,
) -> Result<String, String> {
    let slug: String = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if slug.is_empty() {
        return Err("worktree name must contain letters or digits".into());
    }
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    let rel = format!(".z-engine/worktrees/{slug}");
    git(
        &root,
        &["worktree", "add", &rel, "-b", &format!("zengine/{slug}")],
    )
    .map_err(|e| format!("git worktree add failed: {e}"))?;

    // Keep the worktree invisible to `git status`.
    let exclude = root.join(".git/info/exclude");
    let line = format!("/{rel}");
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    if !existing.lines().any(|l| l.trim() == line) {
        // Guard the separator: appending onto a file without a trailing
        // newline would glue the entry onto the last existing line.
        let mut sep = "";
        if !existing.is_empty() && !existing.ends_with('\n') {
            sep = "\n";
        }
        let _ = std::fs::write(&exclude, format!("{existing}{sep}{line}\n"));
    }

    let abs = std::fs::canonicalize(root.join(&rel)).map_err(|e| e.to_string())?;
    let abs_str = abs.to_string_lossy().into_owned();
    let mut roots = load_workspaces();
    if !roots.contains(&abs) {
        roots.push(abs);
        save_workspaces(&roots)?;
    }
    Ok(abs_str)
}

/// Defense-in-depth for IPC commands that take filesystem paths from the
/// webview: canonicalize and require the result to stay under `base`.
/// A compromised webview must not be able to delete or read arbitrary
/// files by passing `..` segments or symlinks pointing elsewhere.
pub(crate) fn contain(base: &Path, candidate: &str) -> Result<PathBuf, String> {
    let joined = base.join(candidate);
    let canon = std::fs::canonicalize(&joined).map_err(|e| format!("{}: {e}", joined.display()))?;
    let base_canon = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    if !canon.starts_with(&base_canon) {
        return Err(format!("path escapes the session store: {candidate}"));
    }
    Ok(canon)
}
