//! Desktop shell (Tauri 2) wrapping the harness-core brain.
//!
//! Serving model (rebuilt from scratch): a minimal HTTP server bound to
//! 127.0.0.1:<random port> serves the built frontend from disk, and the
//! main window is created programmatically pointed at that http:// URL.
//! No alternate schemes, no config-relative asset resolution.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use harness_core::agent::{AgentHandle, EventRx, spawn_with_recorder};
use harness_core::config::{CliOverrides, Config};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Shared application state managed by Tauri.
#[derive(Default)]
struct GuiState {
    handle: Mutex<Option<AgentHandle>>,
    ctx: Mutex<Option<AppCtx>>,
}

#[derive(Clone)]
struct AppCtx {
    project_root: PathBuf,
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

fn build_loop_config(cfg: &Config, project_root: &Path) -> harness_core::agent::LoopConfig {
    harness_core::agent::LoopConfig {
        model: cfg.model.clone(),
        base_url: cfg.base_url.clone(),
        api_key: resolve_api_key(),
        project_root: project_root.to_path_buf(),
        tmp_dir: std::env::temp_dir(),
        initial_allow_rules: cfg.permissions.allow.clone(),
        max_context_tokens: cfg.max_context_tokens,
        keep_recent_messages: 12,
        review_enabled: cfg.review_enabled,
        mcp_servers: cfg.mcp_servers.clone(),
        auto_allow_tools: vec![],
        initial_mode: harness_core::agent::PermissionMode::Normal,
    }
}

fn sessions_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("sessions")
}

fn forward_events(mut rx: EventRx, window: tauri::WebviewWindow) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let payload = serde_json::to_value(&ev).unwrap_or(serde_json::Value::Null);
            if window.emit("appEvent", payload).is_err() {
                break;
            }
        }
    });
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
fn frontend_ready() {
    eprintln!("[gui] frontend mounted");
    tracing::info!("frontend mounted");
}

#[tauri::command]
fn submit(text: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.submit(text);
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

#[derive(serde::Serialize, Debug)]
pub struct SessionEntry {
    pub path: String,
    pub ulid: String,
    pub first_user_msg: Option<String>,
    pub modified_ms: u128,
}

#[tauri::command]
fn list_sessions() -> Result<Vec<SessionEntry>, String> {
    use std::time::UNIX_EPOCH;
    Ok(harness_core::session::list_sessions(&sessions_dir())
        .into_iter()
        .map(|s| SessionEntry {
            modified_ms: s
                .modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            path: s.path.to_string_lossy().into_owned(),
            ulid: s.ulid,
            first_user_msg: s.first_user_msg,
        })
        .collect())
}

#[tauri::command]
fn delete_session(path: String) -> Result<(), String> {
    harness_core::session::delete_session(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_permission_rules(state: tauri::State<'_, GuiState>) -> Result<Vec<String>, String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    harness_core::config::list_bash_rules(&ctx.project_root).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_permission_rule(rule: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    harness_core::config::persist_bash_rule(&ctx.project_root, &rule)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_permission_rule(rule: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    harness_core::config::remove_bash_rule(&ctx.project_root, &rule).map_err(|e| e.to_string())
}

#[tauri::command]
fn start_session(
    resume_path: Option<String>,
    state: tauri::State<'_, GuiState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    {
        let guard = state.handle.lock().map_err(|_| "state poisoned")?;
        if let Some(h) = guard.as_ref() {
            h.shutdown();
        }
    }

    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    let cfg =
        Config::load(&Default::default(), Some(&ctx.project_root)).map_err(|e| e.to_string())?;
    let lc = build_loop_config(&cfg, &ctx.project_root);

    let recorder: Option<harness_core::session::SessionWriter>;
    let recorder_path: Option<PathBuf>;
    let resume_state;
    match &resume_path {
        Some(p) => {
            let events =
                harness_core::session::read_events(Path::new(p)).map_err(|e| e.to_string())?;
            let replayed = harness_core::session::replay(&events);
            resume_state = Some(harness_core::agent::ResumeState {
                working: replayed.working,
                note_payloads: replayed.notes_replayed,
            });
            let w = harness_core::session::SessionWriter::append_to(Path::new(p))
                .map_err(|e| e.to_string())?;
            recorder_path = Some(w.path.clone());
            recorder = Some(w);
        }
        None => {
            resume_state = None;
            let w = harness_core::session::SessionWriter::create(&sessions_dir())
                .map_err(|e| e.to_string())?;
            recorder_path = Some(w.path.clone());
            recorder = Some(w);
        }
    }

    let (handle, ev_rx) = spawn_with_recorder(lc, resume_state, recorder);
    *state.handle.lock().map_err(|_| "state poisoned")? = Some(handle);

    let window = app
        .get_webview_window("main")
        .ok_or("main window missing")?;
    forward_events(ev_rx, window);

    let ulid = recorder_path
        .as_ref()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    app.emit("sessionChanged", json!({ "ulid": ulid }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- local HTTP asset server ----------------------------------------------

fn main() {
    // App-lifetime tokio runtime entered on the main thread so agent
    // startup `tokio::spawn`s land on a real reactor under Tauri.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _enter = rt.enter(); // intentionally lives for the process

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
                .expect("cannot open log"),
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
        .invoke_handler(tauri::generate_handler![
            frontend_ready,
            submit,
            abort,
            compact,
            notes,
            set_mode,
            set_model,
            approve_with_rule,
            deny,
            list_sessions,
            delete_session,
            list_permission_rules,
            save_permission_rule,
            remove_permission_rule,
            start_session
        ])
        .setup(|app| {
            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let cfg = Config::load(&CliOverrides::default(), Some(&project_root))
                .map_err(|e| e.to_string())?;
            let lc = build_loop_config(&cfg, &project_root);

            let (handle, ev_rx) = spawn_with_recorder(lc, None, None);
            {
                let st = app.state::<GuiState>();
                *st.handle.lock().unwrap() = Some(handle);
                *st.ctx.lock().unwrap() = Some(AppCtx {
                    project_root: project_root.clone(),
                });
            }

            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("harness")
            .inner_size(1100.0, 760.0)
            .min_inner_size(720.0, 520.0)
            .build()
            .map_err(|e| e.to_string())?;

            forward_events(ev_rx, window);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running harness GUI");
}
