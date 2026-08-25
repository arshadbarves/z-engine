//! Desktop shell (Tauri 2) wrapping the harness-core brain.
//!
//! Owns the `AgentHandle`, forwards core events to the webview, and
//! exposes typed commands. Presentation lives entirely in the Svelte app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use harness_core::agent::{AgentHandle, EventRx, spawn_with_recorder};
use harness_core::config::{CliOverrides, Config};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Shared application state managed by Tauri.
#[derive(Default)]
struct GuiState {
    handle: Mutex<Option<AgentHandle>>,
}

fn resolve_api_key() -> Option<String> {
    if let Ok(k) = std::env::var("HARNESS_API_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Some(k);
        }
    }
    let path = dirs::home_dir()?
        .join(".config")
        .join("harness")
        .join("api-key");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
fn submit(text: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    let handle = guard.as_ref().ok_or("agent not started")?;
    handle.submit(text);
    Ok(())
}

#[tauri::command]
fn abort(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.abort();
    Ok(())
}

#[tauri::command]
fn compact(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.compact();
    Ok(())
}

fn forward_events(mut rx: EventRx, window: tauri::WebviewWindow) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let payload = serde_json::to_value(&ev).unwrap_or(serde_json::Value::Null);
            if window.emit("appEvent", payload).is_err() {
                break; // window gone
            }
        }
    });
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(
                    dirs::data_dir()
                        .unwrap_or_else(|| PathBuf::from("/tmp"))
                        .join("harness/harness-gui.log"),
                )
                .unwrap_or_else(|e| panic!("cannot open log: {e}")),
        ))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .try_init()
        .ok();

    tauri::Builder::default()
        .manage(GuiState::default())
        .setup(|app| {
            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let cfg = Config::load(&CliOverrides::default(), Some(&project_root))
                .map_err(|e| e.to_string())?;
            let api_key = resolve_api_key();

            let lc = harness_core::agent::LoopConfig {
                model: cfg.model.clone(),
                base_url: cfg.base_url.clone(),
                api_key,
                project_root: project_root.clone(),
                tmp_dir: std::env::temp_dir(),
                initial_allow_rules: cfg.permissions.allow.clone(),
                max_context_tokens: cfg.max_context_tokens,
                keep_recent_messages: 12,
                review_enabled: cfg.review_enabled,
                mcp_servers: cfg.mcp_servers.clone(),
                auto_allow_tools: vec![],
                initial_mode: harness_core::agent::PermissionMode::Normal,
            };

            let (handle, ev_rx) = spawn_with_recorder(lc, None, None);
            *app.state::<GuiState>().handle.lock().unwrap() = Some(handle);

            let window = app
                .get_webview_window("main")
                .ok_or("main window missing")?;
            forward_events(ev_rx, window);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![submit, abort, compact])
        .run(tauri::generate_context!())
        .expect("error while running harness GUI");
}
