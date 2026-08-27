use crate::state::GuiState;
use serde_json::json;
use std::path::PathBuf;
use z_engine_core::config::Config;

#[tauri::command]
pub(crate) fn set_model(model: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard
        .as_ref()
        .ok_or("agent not started")?
        .set_model(model.clone());
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
    let cfg =
        Config::load(&Default::default(), Some(&ctx.project_root)).map_err(|e| e.to_string())?;
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
    let cost_overrides: serde_json::Map<String, serde_json::Value> = cfg
        .cost_overrides
        .iter()
        .map(|(m, p)| {
            (
                m.clone(),
                json!({
                    "usdPerMtokInput": p.usd_per_mtok_input,
                    "usdPerMtokOutput": p.usd_per_mtok_output,
                }),
            )
        })
        .collect();
    Ok(json!({
        "model": model,
        "maxContextTokens": cfg.max_context_tokens,
        "maxOutputTokens": cfg.max_output_tokens,
        "compactAtPercent": cfg.compact_at_percent,
        "baseUrl": cfg.base_url,
        "reviewEnabled": cfg.review_enabled,
        "pricing": pricing,
        "mcpServers": mcp_servers,
        "costOverrides": cost_overrides,
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
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn save_general(
    model: Option<String>,
    base_url: Option<String>,
    max_context_tokens: Option<u32>,
    review: Option<bool>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let over = z_engine_core::config::GeneralOverrides {
        model: model.clone(),
        base_url,
        max_context_tokens,
        review_enabled: review,
    };
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    z_engine_core::config::persist_general(&ctx.project_root, &over).map_err(|e| e.to_string())?;

    if let Some(m) = model {
        if let Some(h) = state.handle.lock().map_err(|_| "state poisoned")?.as_ref() {
            h.set_model(m.clone());
        }
        *state.model.lock().map_err(|_| "state poisoned")? = m;
    }
    Ok(())
}

/// Settings → Cost: per-model USD/MTok override persisted to
/// `.z-engine/config.toml` under `[cost.overrides]`.
#[tauri::command]
pub(crate) fn set_cost_override(
    model: String,
    usd_per_mtok_input: f64,
    usd_per_mtok_output: f64,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    z_engine_core::config::set_cost_override(
        &ctx.project_root,
        &model,
        z_engine_core::context::cost::Pricing {
            usd_per_mtok_input,
            usd_per_mtok_output,
        },
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_cost_override(
    model: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    z_engine_core::config::remove_cost_override(&ctx.project_root, &model)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_permission_rules(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
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
    z_engine_core::config::remove_bash_rule(&ctx.project_root, &rule).map_err(|e| e.to_string())
}

/// Resolved MCP server table for the Settings tab.
#[tauri::command]
pub(crate) fn list_mcp_servers(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<serde_json::Value>, String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    let cfg =
        Config::load(&Default::default(), Some(&ctx.project_root)).map_err(|e| e.to_string())?;
    Ok(cfg
        .mcp_servers
        .iter()
        .map(|s| json!({ "name": s.name, "command": s.command, "args": s.args }))
        .collect())
}

/// Settings → MCP Test button: spawn the server, handshake, tools/list.
/// Returns tool names; the connection is dropped afterwards.
#[tauri::command]
pub(crate) async fn test_mcp_server(name: String) -> Result<Vec<String>, String> {
    use z_engine_core::mcp::McpConnection;
    // Resolve the server definition from layered config.
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = Config::load(&Default::default(), Some(&project_root)).map_err(|e| e.to_string())?;
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
