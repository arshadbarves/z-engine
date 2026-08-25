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

#[tauri::command]
fn notes(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.request_notes();
    Ok(())
}

#[tauri::command]
fn set_mode(mode: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    use harness_core::agent::PermissionMode;
    let m = match mode.as_str() {
        "accept-edits" | "auto-accept edits" => PermissionMode::AutoAcceptEdits,
        "plan" => PermissionMode::Plan,
        _ => PermissionMode::Normal,
    };
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.set_mode(m);
    Ok(())
}

#[tauri::command]
fn set_model(model: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.set_model(model);
    Ok(())
}

/// Decision strings from the frontend: once | session | persist
#[tauri::command]
fn approve(id: u64, decision: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    use harness_core::agent::{ApprovalDecision, PermissionMode};
    let d = match decision.as_str() {
        "session" => ApprovalDecision::AlwaysSession {
            rule: "bash*".into(), // refined below by frontend-provided rule
        },
        "persist" => ApprovalDecision::AlwaysPersist {
            rule: "bash*".into(),
        },
        _ => ApprovalDecision::Once,
    };
    let _ = PermissionMode::Normal; // silence unused in future refactors
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.approve(id, d);
    Ok(())
}

/// Frontend sends the resolved rule alongside so session/persist match the
/// suggested one shown to the user.
#[tauri::command]
fn approve_with_rule(
    id: u64,
    decision: String,
    rule: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    use harness_core::agent::ApprovalDecision;
    let d = match decision.as_str() {
        "session" => ApprovalDecision::AlwaysSession { rule },
        "persist" => ApprovalDecision::AlwaysPersist { rule },
        _ => ApprovalDecision::Once,
    };
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.approve(id, d);
    Ok(())
}

#[tauri::command]
fn deny(id: u64, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.deny(id);
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

/// App-lifetime tokio runtime. Entered once on the main thread so every
/// `tokio::spawn` performed during setup/agent-startup lands on a real
/// reactor (Tauri's own runtime is separate and not installed globally).
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn main() {
    let rt = runtime();
    // Intentionally leaked for the process lifetime.
    let _enter = rt.enter();
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
        .invoke_handler(tauri::generate_handler![
            submit,
            abort,
            compact,
            notes,
            set_mode,
            set_model,
            approve,
            approve_with_rule,
            deny
        ])
        .run(tauri::generate_context!())
        .expect("error while running harness GUI");
}
