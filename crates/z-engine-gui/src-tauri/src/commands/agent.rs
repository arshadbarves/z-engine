use crate::event_bridge::forward_events;
use crate::session_store::{
    StartSessionResult, contain_session, session_events_json, sessions_dir,
};
use crate::state::{GuiState, build_loop_config};
use serde_json::json;
use std::path::PathBuf;
use tauri::{Emitter, Manager};
use z_engine_core::agent::spawn_with_recorder;
use z_engine_core::config::Config;

#[tauri::command]
pub(crate) fn frontend_ready() {
    eprintln!("[gui] frontend mounted");
    tracing::info!("frontend mounted");
}

#[tauri::command]
pub(crate) fn submit(
    text: String,
    images: Option<Vec<String>>,
    session_id: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    state
        .handle_for(session_id.as_deref())?
        .submit_with_images(text, images.unwrap_or_default());
    Ok(())
}

#[tauri::command]
pub(crate) fn abort(
    session_id: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    state.handle_for(session_id.as_deref())?.abort();
    Ok(())
}

#[tauri::command]
pub(crate) fn compact(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.compact();
    Ok(())
}

#[tauri::command]
pub(crate) fn notes(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.request_notes();
    Ok(())
}

#[tauri::command]
pub(crate) fn set_mode(mode: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    use z_engine_core::agent::PermissionMode;
    let m = match mode.as_str() {
        "accept-edits" | "auto-accept edits" => PermissionMode::AutoAcceptEdits,
        "plan" => PermissionMode::Plan,
        _ => PermissionMode::Normal,
    };
    state.handle_for(None)?.set_mode(m);
    Ok(())
}

/// `! <cmd>` shell passthrough — executed locally, never touches the model.
#[tauri::command]
pub(crate) fn shell(cmd: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.shell(cmd);
    Ok(())
}

/// Rewind: restore files touched by the last checkpointed turn.
#[tauri::command]
pub(crate) fn revert_last_turn(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.revert_last_turn();
    Ok(())
}

/// Per-message revert: undo every file change from run-turn `keep`
/// (the user message being reverted) and everything after it.
#[tauri::command]
pub(crate) fn revert_to_turn(keep: u64, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.revert_to_turn(keep);
    Ok(())
}

/// Pick the reasoning effort for reasoning-capable models.
#[tauri::command]
pub(crate) fn set_reasoning_effort(
    effort: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    state.handle_for(None)?.set_reasoning_effort(effort);
    Ok(())
}

#[tauri::command]
pub(crate) fn approve_with_rule(
    id: u64,
    decision: String,
    rule: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    use z_engine_core::agent::ApprovalDecision;
    let d = match decision.as_str() {
        "session" => ApprovalDecision::AlwaysSession { rule },
        "persist" => ApprovalDecision::AlwaysPersist { rule },
        _ => ApprovalDecision::Once,
    };
    state.handle_for(None)?.approve(id, d);
    Ok(())
}

#[tauri::command]
pub(crate) fn deny(id: u64, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    state.handle_for(None)?.deny(id);
    Ok(())
}

#[tauri::command]
pub(crate) fn start_session(
    resume_path: Option<String>,
    root: Option<String>,
    state: tauri::State<'_, GuiState>,
    app: tauri::AppHandle,
) -> Result<StartSessionResult, String> {
    // Snapshot the current workspace root, then release the ctx lock so
    // the whole swap below runs under a single handle-lock section —
    // concurrent "New task" clicks must not interleave shutdown/spawn.
    let base_root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();

    // Per-workspace sessions run against the chosen root (Codex-style
    // projects); tools, config layering and @-file listing follow it.
    let project_root: PathBuf = match &root {
        Some(r) => {
            let canonical = std::fs::canonicalize(r).map_err(|e| format!("{r}: {e}"))?;
            if !canonical.is_dir() {
                return Err(format!("{} is not a directory", canonical.display()));
            }
            canonical
        }
        None => base_root,
    };
    let cfg = Config::load(&Default::default(), Some(&project_root)).map_err(|e| e.to_string())?;
    let lc = build_loop_config(&cfg, &project_root);

    let recorder: Option<z_engine_core::session::SessionWriter>;
    let recorder_path: Option<PathBuf>;
    let resume_state;
    let mut ui_events: Vec<serde_json::Value> = Vec::new();
    match &resume_path {
        Some(p) => {
            let contained = contain_session(p)?;
            let ulid = contained
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let events =
                z_engine_core::session::read_events(&contained).map_err(|e| e.to_string())?;
            ui_events = session_events_json(&events);
            if !ulid.is_empty() && state.has_loop(&ulid)? {
                state.set_active(ulid.clone())?;
                if root.is_some() {
                    let mut ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
                    if let Some(c) = ctx_guard.as_mut() {
                        c.project_root = project_root;
                    }
                }
                app.emit("sessionChanged", json!({ "ulid": ulid }))
                    .map_err(|e| e.to_string())?;
                return Ok(StartSessionResult {
                    ulid,
                    events: ui_events,
                    already_live: true,
                    path: Some(contained.to_string_lossy().into_owned()),
                });
            }
            let replayed = z_engine_core::session::replay(&events);
            resume_state = Some(z_engine_core::agent::ResumeState {
                working: replayed.working,
                note_payloads: replayed.notes_replayed,
            });
            let w = z_engine_core::session::SessionWriter::append_to(&contained)
                .map_err(|e| e.to_string())?;
            recorder_path = Some(w.path.clone());
            recorder = Some(w);
        }
        None => {
            resume_state = None;
            let mut w = z_engine_core::session::SessionWriter::create(&sessions_dir())
                .map_err(|e| e.to_string())?;
            // Record the environment up front — the sidebar groups sessions
            // under their workspace via this Meta event's project_root.
            let _ = w.record(&z_engine_core::session::SessionEvent::Meta {
                model: lc.model.clone(),
                project_root: project_root.to_string_lossy().into_owned(),
            });
            recorder_path = Some(w.path.clone());
            recorder = Some(w);
        }
    }

    let ulid = recorder_path
        .as_ref()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();

    let (_handle, ev_rx) = spawn_with_recorder(lc, resume_state, recorder);
    state.insert_loop(ulid.clone(), _handle)?;
    let _ = state.shutdown_one("boot");

    // Follow the chosen workspace for @-files, config and rule persistence.
    if root.is_some() {
        let mut ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
        if let Some(c) = ctx_guard.as_mut() {
            c.project_root = project_root;
        }
    }
    *state.model.lock().map_err(|_| "state poisoned")? = cfg.model.clone();

    let window = app
        .get_webview_window("main")
        .ok_or("main window missing")?;
    forward_events(ev_rx, window, ulid.clone());

    app.emit("sessionChanged", json!({ "ulid": ulid }))
        .map_err(|e| e.to_string())?;
    Ok(StartSessionResult {
        ulid,
        events: ui_events,
        already_live: false,
        path: recorder_path.map(|p| p.to_string_lossy().into_owned()),
    })
}

/// Last assembled chat-completion request for the prompt inspector.
#[tauri::command]
pub(crate) fn inspect_prompt(
    session_id: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<z_engine_core::agent::PromptInspect, String> {
    state
        .handle_for(session_id.as_deref())?
        .last_prompt()
        .ok_or_else(|| "no prompt captured yet".into())
}
