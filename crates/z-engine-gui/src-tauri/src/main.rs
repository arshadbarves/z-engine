//! Desktop shell (Tauri 2) wrapping the z-engine-core brain.
//!
//! Serving model (rebuilt from scratch): a minimal HTTP server bound to
//! 127.0.0.1:<random port> serves the built frontend from disk, and the
//! main window is created programmatically pointed at that http:// URL.
//! No alternate schemes, no config-relative asset resolution.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod commands;
mod event_bridge;
mod git_util;
mod session_store;
mod slash_commands;
mod state;

use event_bridge::forward_events;
use state::{AppCtx, GuiState, build_loop_config};
use std::path::PathBuf;
use tauri::Manager;
use z_engine_core::agent::spawn_with_recorder;
use z_engine_core::config::{CliOverrides, Config};

fn main() {
    // App-lifetime tokio runtime entered on the main thread so agent
    // startup `tokio::spawn`s land on a real reactor under Tauri.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _enter = rt.enter(); // intentionally lives for the process

    // Log file lives under <data_dir>/z-engine/; the directory may not
    // exist on first run, and a logging failure must never block launch.
    let log_path = z_engine_core::config::app_data_write_dir().join("z-engine-gui.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_writer(std::sync::Mutex::new(file))
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .without_time()
                .try_init()
                .ok();
        }
        Err(e) => {
            eprintln!("z-engine-gui: cannot open log {}: {e}", log_path.display());
        }
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GuiState::default())
        .invoke_handler(tauri::generate_handler![
            commands::frontend_ready,
            commands::submit,
            commands::abort,
            commands::compact,
            commands::notes,
            commands::set_mode,
            commands::set_model,
            commands::approve_with_rule,
            commands::deny,
            commands::list_sessions,
            commands::delete_session,
            commands::list_workspaces,
            commands::add_workspace,
            commands::remove_workspace,
            commands::fetch_model_catalog,
            commands::set_reasoning_effort,
            commands::list_slash_commands,
            commands::read_slash_command,
            commands::list_changed_files,
            commands::diff_for_file,
            commands::create_worktree,
            commands::list_permission_rules,
            commands::save_permission_rule,
            commands::remove_permission_rule,
            commands::read_session,
            commands::save_general,
            commands::set_cost_override,
            commands::remove_cost_override,
            commands::list_mcp_servers,
            commands::test_mcp_server,
            commands::list_project_files,
            commands::get_config,
            commands::shell,
            commands::revert_last_turn,
            commands::revert_to_turn,
            commands::start_session
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
                *st.model.lock().unwrap() = cfg.model.clone();
            }

            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Z Engine")
            .inner_size(1100.0, 760.0)
            .min_inner_size(720.0, 520.0)
            // Codex-desktop chrome: no separate title bar — traffic lights
            // float over the sidebar (which pads for them).
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .maximized(true)
            .build()
            .map_err(|e| e.to_string())?;

            forward_events(ev_rx, window);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Z Engine GUI");
    app.run(|_app_handle, event| {
        // Tear the agent down on quit so bash/MCP child processes don't
        // outlive the closed window as orphans.
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            if let Some(st) = _app_handle.try_state::<GuiState>() {
                if let Ok(mut guard) = st.handle.lock() {
                    if let Some(h) = guard.take() {
                        h.shutdown();
                    }
                }
            }
        }
    });
}
