//! Workspace commands (`lib/commands/workspace.ts`): the registry, the
//! git-scope review panel, and worktrees. Git commands act on the active
//! project (the last one opened or added).

use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::State;

use crate::ipc::{IpcResult, fail, json};
use crate::state::AppState;

#[tauri::command]
pub(crate) fn list_workspaces(state: State<'_, AppState>) -> Vec<String> {
    state
        .workspaces
        .load()
        .iter()
        .map(|root| display(root))
        .collect()
}

/// Registers an existing directory (duplicates are ignored), makes it the
/// active project, and returns the canonical path stored.
#[tauri::command]
pub(crate) fn add_workspace(path: String, state: State<'_, AppState>) -> IpcResult<String> {
    let canonical = std::fs::canonicalize(&path).map_err(|error| format!("{path}: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    state.workspaces.add(canonical.clone())?;
    state.set_active(canonical.clone());
    Ok(display(&canonical))
}

/// Unregisters a workspace and deletes its chats.
#[tauri::command]
pub(crate) async fn remove_workspace(path: String, state: State<'_, AppState>) -> IpcResult<()> {
    let target = PathBuf::from(&path);
    let canonical = std::fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
    let targets = [target, canonical];
    let sessions = state.engine.list_sessions().map_err(fail)?;
    for session in sessions {
        let root = PathBuf::from(&session.project_root);
        if targets.contains(&root) {
            state
                .engine
                .delete_session(&session.session_id)
                .await
                .map_err(fail)?;
        }
    }
    state.workspaces.remove(&targets)
}

#[tauri::command]
pub(crate) async fn list_changed_files(state: State<'_, AppState>) -> IpcResult<Value> {
    let root = state.active_root();
    json(state.engine.git_changed_files(&root).await.map_err(fail)?)
}

#[tauri::command]
pub(crate) async fn diff_for_file(path: String, state: State<'_, AppState>) -> IpcResult<String> {
    let root = state.active_root();
    state.engine.git_file_diff(&root, &path).await.map_err(fail)
}

/// Creates `.z-engine/worktrees/<name>` in the active project on its own
/// branch and registers it as a workspace.
#[tauri::command]
pub(crate) async fn create_worktree(name: String, state: State<'_, AppState>) -> IpcResult<String> {
    let root = state.active_root();
    let path = state
        .engine
        .create_worktree(&root, &name)
        .await
        .map_err(|error| format!("git worktree add failed: {error}"))?;
    state.workspaces.add(path.clone())?;
    Ok(display(&path))
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
