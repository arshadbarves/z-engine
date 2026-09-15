use crate::state::GuiState;
use serde_json::json;
use z_engine_core::config::Config;

#[tauri::command]
pub(crate) fn set_model(model: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.set_model(model.clone());
    *state.model.lock().map_err(|_| "state poisoned")? = model;
    Ok(())
}

/// Current agent-facing configuration for UI chrome (model picker,
/// context meter, cost estimate, settings tabs).
#[tauri::command]
pub(crate) fn get_config(state: tauri::State<'_, GuiState>) -> Result<serde_json::Value, String> {
    let model = state.model.lock().map_err(|_| "state poisoned")?.clone();
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = ctx_guard.as_ref() else {
        return Err("not initialized".into());
    };
    let cfg = Config::load(Some(&ctx.project_root)).map_err(|e| e.to_string())?;
    let key_st = z_engine_core::config::current_key_status_for_base_url(&cfg.base_url);
    let pricing = cfg.pricing_for(&model).map(|p| {
        json!({
            "usdPerMtokInput": p.usd_per_mtok_input,
            "usdPerMtokOutput": p.usd_per_mtok_output,
        })
    });
    let mcp_servers: Vec<serde_json::Value> = cfg
        .mcp_servers
        .iter()
        .map(|s| json!({ "name": s.name, "command": s.command, "args": s.args }))
        .collect();
    Ok(json!({
        "model": model,
        "maxContextTokens": cfg.max_context_tokens,
        "maxOutputTokens": cfg.max_output_tokens,
        "compactAtPercent": cfg.compact_at_percent,
        "baseUrl": cfg.base_url,
        "reviewEnabled": cfg.review_enabled,
        "maxTaskContinuations": cfg.max_task_continuations,
        "taskReportView": cfg.task_report_view,
        "hasApiKey": key_st.has_key,
        "apiKeyHint": key_st.hint,
        "pricing": pricing,
        "mcpServers": mcp_servers,
        "version": env!("CARGO_PKG_VERSION"),
        "projectName": ctx
            .project_root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ctx.project_root.to_string_lossy().into_owned()),
    }))
}

/// Settings → General: persist scalars into `.z-engine/config.toml` and
/// hot-apply the model to the running agent when one exists.
fn persist_project_general_if_valid(
    project_root: Option<&std::path::Path>,
    over: &z_engine_core::config::GeneralOverrides,
) -> Result<(), String> {
    let Some(project_root) = project_root.filter(|root| crate::state::is_valid_project_root(root))
    else {
        return Ok(());
    };
    z_engine_core::config::persist_general(project_root, over)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `task_report_view` is a global presentation preference: it follows the
/// reader across workspaces and must never land in a project's shared
/// `.z-engine/config.toml`.
fn project_scoped_general(
    over: &z_engine_core::config::GeneralOverrides,
) -> z_engine_core::config::GeneralOverrides {
    z_engine_core::config::GeneralOverrides {
        task_report_view: None,
        ..over.clone()
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn save_general(
    model: Option<String>,
    base_url: Option<String>,
    max_context_tokens: Option<u32>,
    review: Option<bool>,
    max_task_continuations: Option<u32>,
    task_report_view: Option<z_engine_core::config::TaskReportView>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let over = z_engine_core::config::GeneralOverrides {
        model: model.clone(),
        base_url: base_url.clone(),
        max_context_tokens,
        review_enabled: review,
        max_task_continuations,
        task_report_view,
    };
    // Persist to global config so default preferences stick across sessions & workspaces.
    z_engine_core::config::persist_global_general(&over).map_err(|e| e.to_string())?;

    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    if let Some(ctx) = ctx_guard.as_ref() {
        persist_project_general_if_valid(Some(&ctx.project_root), &project_scoped_general(&over))?;
    }
    drop(ctx_guard);

    if let Some(m) = model {
        if let Ok(h) = state.handle_for(None) {
            h.set_model(m.clone());
        }
        *state.model.lock().map_err(|_| "state poisoned")? = m;
    }
    if let Some(base_url) = base_url {
        let api_key = z_engine_core::config::resolve_api_key_for(&base_url);
        let loops = state.loops.lock().map_err(|_| "state poisoned")?;
        for handle in loops.values() {
            handle.set_provider(base_url.clone(), api_key.clone());
        }
    }
    Ok(())
}

/// Settings → General: persist the active provider key to `auth.json` and
/// hot-apply it to every running agent loop.
#[tauri::command]
pub(crate) fn save_api_key(
    key: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    z_engine_core::config::ensure_user_config().map_err(|e| e.to_string())?;
    let trimmed = key.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let base_url = ctx_guard
        .as_ref()
        .and_then(|ctx| {
            Config::load(Some(&ctx.project_root))
                .ok()
                .map(|c| c.base_url)
        })
        .unwrap_or_else(|| Config::default().base_url);
    drop(ctx_guard);
    z_engine_core::config::set_current_key_for_base_url(&base_url, trimmed)
        .map_err(|e| e.to_string())?;
    let loops = state.loops.lock().map_err(|_| "state poisoned")?;
    for h in loops.values() {
        h.set_api_key(trimmed.map(str::to_string));
    }
    Ok(())
}

/// Settings → MCP: add or replace a stdio server in project config.
#[tauri::command]
pub(crate) fn save_mcp_server(
    name: String,
    command: String,
    args: Vec<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    if !crate::state::is_valid_project_root(&ctx.project_root) {
        return Err("No active workspace folder".into());
    }
    z_engine_core::config::persist_mcp_server(&ctx.project_root, &name, &command, args)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_mcp_server(
    name: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    if !crate::state::is_valid_project_root(&ctx.project_root) {
        return Ok(());
    }
    z_engine_core::config::remove_mcp_server(&ctx.project_root, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_permission_rules(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    if !crate::state::is_valid_project_root(&ctx.project_root) {
        return Ok(Vec::new());
    }
    z_engine_core::config::list_bash_rules(&ctx.project_root).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn save_permission_rule(
    rule: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    if !crate::state::is_valid_project_root(&ctx.project_root) {
        return Err("No active workspace folder".into());
    }
    z_engine_core::config::persist_bash_rule(&ctx.project_root, &rule)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_permission_rule(
    rule: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    if !crate::state::is_valid_project_root(&ctx.project_root) {
        return Ok(());
    }
    z_engine_core::config::remove_bash_rule(&ctx.project_root, &rule).map_err(|e| e.to_string())
}

/// Resolved MCP server table for the Settings tab.
#[tauri::command]
pub(crate) fn list_mcp_servers(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<serde_json::Value>, String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    let cfg = Config::load(Some(&ctx.project_root)).map_err(|e| e.to_string())?;
    Ok(cfg
        .mcp_servers
        .iter()
        .map(|s| json!({ "name": s.name, "command": s.command, "args": s.args }))
        .collect())
}

/// Settings → MCP Test button: spawn the server, handshake, tools/list.
/// Returns tool names; the connection is dropped afterwards.
#[tauri::command]
pub(crate) async fn test_mcp_server(
    name: String,
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    use z_engine_core::mcp::McpConnection;
    let project_root = {
        let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
        ctx_guard
            .as_ref()
            .ok_or_else(|| "not initialized".to_string())?
            .project_root
            .clone()
    };
    let cfg = Config::load(Some(&project_root)).map_err(|e| e.to_string())?;
    let srv = cfg
        .mcp_servers
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("no mcp server named '{name}'"))?;
    let conn = McpConnection::new(&srv.name, &srv.command, &srv.args, &project_root);
    conn.ensure().await?;
    Ok(conn
        .list_tools()
        .await
        .into_iter()
        .map(|t| t.name)
        .collect())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
