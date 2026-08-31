use crate::state::{GuiState, load_workspaces, save_workspaces};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use z_engine_core::tools::unified_diff;

// ---- diff review (changed files vs HEAD) ------------------------------------

#[derive(serde::Serialize)]
pub(crate) struct ChangedFile {
    path: String,
    status: String,
    added: u32,
    deleted: u32,
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

/// Files this chat mutated (checkpoint baseline → disk), for the review panel.
#[tauri::command]
pub(crate) fn list_session_changed_files(
    session_id: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<ChangedFile>, String> {
    let Ok(handle) = state.handle_for(session_id.as_deref()) else {
        return Ok(Vec::new()); // no live agent → nothing chat-scoped yet
    };
    Ok(handle
        .session_changed_files()
        .into_iter()
        .map(|f| ChangedFile {
            path: f.path,
            status: f.status,
            added: f.added,
            deleted: f.deleted,
        })
        .collect())
}

/// Unified diff for one path vs this chat's checkpoint pre-image.
#[tauri::command]
pub(crate) fn session_diff_for_file(
    path: String,
    session_id: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<String, String> {
    state
        .handle_for(session_id.as_deref())
        .map_err(|_| "no live chat to diff against".to_string())?
        .session_diff_for_file(&path)
}

/// Working-tree changes (vs HEAD) for the review panel's optional git scope.
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
    let numstat = numstat_map(&root);

    let mut out = Vec::new();
    // -z output is NUL-separated: XY<space>path\0[orig\0]
    let mut iter = porcelain.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = iter.next() {
        let mut chars = entry.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let rest = chars.as_str();
        let status_ch = if x != ' ' { x } else { y };
        let path = rest.trim_start().to_string();
        // Renames carry "new\0old\0"; keep the new side only.
        if x == 'R' || y == 'R' {
            iter.next();
        }
        let status = match status_ch {
            '?' | 'A' => "added",
            'D' => "deleted",
            'M' | 'C' => "modified",
            'R' => "modified",
            _ => "modified",
        };
        let (added, deleted) = match status {
            "added" => (count_file_lines(&root.join(&path)), 0),
            "deleted" => numstat
                .get(&path)
                .copied()
                .unwrap_or_else(|| (0, count_head_lines(&root, &path))),
            _ => numstat.get(&path).copied().unwrap_or((0, 0)),
        };
        out.push(ChangedFile {
            path,
            status: status.to_string(),
            added,
            deleted,
        });
    }
    Ok(out)
}

fn numstat_map(root: &Path) -> HashMap<String, (u32, u32)> {
    let Ok(raw) = git(root, &["diff", "HEAD", "--numstat"]) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for line in raw.lines() {
        let mut cols = line.splitn(3, '\t');
        let Some(a) = cols.next() else { continue };
        let Some(d) = cols.next() else { continue };
        let Some(path) = cols.next() else { continue };
        // Binary files report "-" for both sides.
        let added = a.parse::<u32>().unwrap_or(0);
        let deleted = d.parse::<u32>().unwrap_or(0);
        // Renames: "old => new" — keep the new side.
        let path = path
            .rsplit_once(" => ")
            .map(|(_, neu)| neu.trim_end_matches('}').trim())
            .unwrap_or(path);
        out.insert(path.to_string(), (added, deleted));
    }
    out
}

fn count_file_lines(path: &Path) -> u32 {
    std::fs::read_to_string(path)
        .map(|s| s.lines().count() as u32)
        .unwrap_or(0)
}

fn count_head_lines(root: &Path, path: &str) -> u32 {
    git(root, &["show", &format!("HEAD:{path}")])
        .map(|s| s.lines().count() as u32)
        .unwrap_or(0)
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
    let joined = root.join(&path);
    let tracked = git(&root, &["ls-files", "--error-unmatch", &path]).is_ok();
    if tracked {
        // Deleted tracked files still have a git diff vs HEAD.
        return git(&root, &["diff", "HEAD", "--", &path]);
    }
    // Untracked / created: full-file addition with real @@ hunks.
    let content = if joined.exists() {
        let resolved = std::fs::canonicalize(&joined).map_err(|e| format!("{path}: {e}"))?;
        if !resolved.starts_with(&root) {
            return Err(format!("path escapes the workspace: {path}"));
        }
        std::fs::read_to_string(&resolved).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(unified_diff("", &content, &path))
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
    let canon_cmp = strip_verbatim(&canon);
    let base_cmp = strip_verbatim(&base_canon);
    if !canon_cmp.starts_with(&base_cmp) {
        return Err(format!("path escapes the session store: {candidate}"));
    }
    Ok(canon)
}

fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}
