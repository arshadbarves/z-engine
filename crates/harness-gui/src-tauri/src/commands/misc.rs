use crate::state::{GuiState, load_workspaces, save_workspaces};
use std::path::{Path, PathBuf};

const WALK_SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".harness",
    ".venv",
    "__pycache__",
];
const WALK_MAX_ENTRIES: usize = 20_000;
const FILE_MATCH_CAP: usize = 200;

/// `@file` picker backend: gitignore-lite project walk filtered by a
/// case-insensitive substring query on the relative path.
#[tauri::command]
pub(crate) fn list_project_files(
    query: String,
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let root = ctx_guard
        .as_ref()
        .map(|c| c.project_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let q = query.to_lowercase();
    let mut out = Vec::new();
    let mut visited = 0usize;
    walk_files(&root, &root, &q, &mut out, &mut visited);
    out.sort_by_key(|p| (p.matches('/').count(), p.clone()));
    out.truncate(FILE_MATCH_CAP);
    Ok(out)
}

fn walk_files(root: &Path, dir: &Path, q: &str, out: &mut Vec<String>, visited: &mut usize) {
    if *visited >= WALK_MAX_ENTRIES || out.len() >= FILE_MATCH_CAP * 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        *visited += 1;
        if *visited >= WALK_MAX_ENTRIES || out.len() >= FILE_MATCH_CAP * 4 {
            return;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            if !WALK_SKIP_DIRS.contains(&name_lossy.as_ref()) && !name_lossy.starts_with('.') {
                walk_files(root, &path, q, out, visited);
            }
            continue;
        }
        if name_lossy.starts_with('.') {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        if q.is_empty() || rel.to_lowercase().contains(q) {
            out.push(rel);
        }
    }
}

#[tauri::command]
pub(crate) fn list_workspaces() -> Vec<String> {
    load_workspaces()
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Register a folder as a workspace (Codex "Open folder"). The path must
/// be an existing directory; duplicates are ignored. Returns the
/// canonical path actually stored.
#[tauri::command]
pub(crate) fn add_workspace(path: String) -> Result<String, String> {
    let canonical = std::fs::canonicalize(&path).map_err(|e| format!("{path}: {e}"))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    let mut roots = load_workspaces();
    if !roots.contains(&canonical) {
        roots.push(canonical.clone());
        save_workspaces(&roots)?;
    }
    Ok(canonical.to_string_lossy().into_owned())
}

#[tauri::command]
pub(crate) fn remove_workspace(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    let mut roots: Vec<PathBuf> = load_workspaces()
        .into_iter()
        .filter(|r| *r != target)
        .collect();
    // Also drop entries that refer to the same dir through a different path.
    if let Ok(canon) = std::fs::canonicalize(&target) {
        roots.retain(|r| *r != canon);
    }
    save_workspaces(&roots)
}
