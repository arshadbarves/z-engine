//! Catalogs the composer and pickers read: slash commands, agents, `@file`
//! search, and the model catalog.

use serde_json::Value;
use tauri::State;

use crate::ipc::{IpcResult, fail, json};
use crate::state::AppState;

#[tauri::command]
pub(crate) fn list_commands(project_root: String, state: State<'_, AppState>) -> IpcResult<Value> {
    let root = AppState::root_arg(&project_root)?;
    json(state.engine.slash_commands(&root))
}

#[tauri::command]
pub(crate) fn list_agents(project_root: String, state: State<'_, AppState>) -> IpcResult<Value> {
    let root = AppState::root_arg(&project_root)?;
    json(state.engine.agent_cards(&root))
}

#[tauri::command]
pub(crate) async fn list_files(
    project_root: String,
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> IpcResult<Vec<String>> {
    let root = AppState::root_arg(&project_root)?;
    state
        .engine
        .list_files(&root, &query, limit)
        .await
        .map_err(fail)
}

/// `ModelInfo[]`: the cached catalog, downloaded when there is none yet.
#[tauri::command]
pub(crate) async fn fetch_model_catalog(state: State<'_, AppState>) -> IpcResult<Value> {
    let catalog = match state.engine.catalog() {
        Some(catalog) => catalog,
        None => state.engine.refresh_catalog().await.map_err(fail)?,
    };
    json(&catalog.models)
}
