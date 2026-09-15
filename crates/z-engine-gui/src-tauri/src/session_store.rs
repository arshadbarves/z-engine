use crate::git_util::contain;
use crate::state::GuiState;
use std::collections::HashMap;
use std::path::PathBuf;
use z_engine_core::session::SessionEvent;
use z_engine_core::verification::{TaskReport, TaskStatus};

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartSessionResult {
    pub ulid: String,
    pub events: Vec<serde_json::Value>,
    pub already_live: bool,
    pub path: Option<String>,
}

pub(crate) fn session_events_json(
    events: &[SessionEvent],
    is_live: bool,
) -> Result<Vec<serde_json::Value>, String> {
    replayed_events(events, is_live)
        .iter()
        .map(|event| serde_json::to_value(event).map_err(|error| error.to_string()))
        .collect()
}

/// A newly spawned loop does not resume an old in-flight operation. Persist
/// this boundary so a later live reattach cannot resurrect its running report.
pub(crate) fn persist_restart_interruptions(
    events: &[SessionEvent],
    path: &std::path::Path,
) -> Result<(), String> {
    let Some(index) = current_task_index(events) else {
        return Ok(());
    };
    if let SessionEvent::TaskUpdated { report } = &events[index] {
        if matches!(
            report.status,
            TaskStatus::Running | TaskStatus::NeedsVerification
        ) {
            let mut report = report.clone();
            interrupt_report(&mut report);
            let mut writer = z_engine_core::session::SessionWriter::append_to(path)
                .map_err(|e| e.to_string())?;
            writer
                .record_durable(&SessionEvent::TaskUpdated { report })
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn replayed_events(events: &[SessionEvent], is_live: bool) -> Vec<SessionEvent> {
    let workspace = events.iter().find_map(|event| match event {
        SessionEvent::Meta { project_root, .. } => Some(project_root.as_str()),
        _ => None,
    });
    let mut latest = HashMap::new();
    for (index, event) in events.iter().enumerate() {
        if let SessionEvent::TaskUpdated { report } = event {
            latest.insert(report.task_id.as_str(), index);
        }
    }
    let active_report = if is_live {
        current_task_index(events)
    } else {
        None
    };
    events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let mut event = event.clone();
            if let SessionEvent::TaskUpdated { report } = &mut event {
                if latest.get(report.task_id.as_str()) == Some(&index) {
                    refresh_replayed_report(report, workspace, active_report == Some(index));
                }
            }
            event
        })
        .collect()
}

fn current_task_index(events: &[SessionEvent]) -> Option<usize> {
    for (index, event) in events.iter().enumerate().rev() {
        match event {
            SessionEvent::UserMsg { .. } => return None,
            SessionEvent::TaskUpdated { .. } => return Some(index),
            _ => {}
        }
    }
    None
}

fn projected_unread_outcome(events: &[SessionEvent], is_live: bool) -> Option<String> {
    let outcome = z_engine_core::session::unread_outcome(events)?;
    if is_live {
        if let Some(index) = current_task_index(events) {
            if let SessionEvent::TaskUpdated { report } = &events[index] {
                match report.status {
                    TaskStatus::Running => return Some("running".into()),
                    TaskStatus::NeedsVerification => return Some("needs_verification".into()),
                    _ => {}
                }
            }
        }
    }
    Some(outcome)
}

fn refresh_replayed_report(report: &mut TaskReport, workspace: Option<&str>, is_active: bool) {
    if !is_active
        && matches!(
            report.status,
            TaskStatus::Running | TaskStatus::NeedsVerification
        )
    {
        interrupt_report(report);
    }
    let boundary = workspace.and_then(|root| std::fs::canonicalize(root).ok());
    let report_root = std::fs::canonicalize(&report.workspace_root).ok();
    if boundary.is_none() || boundary != report_root {
        report.status = TaskStatus::Stale;
        report.blockers.push(
            "Task evidence workspace cannot be validated against the session workspace.".into(),
        );
        return;
    }
    let previous = report.status;
    if let Err(error) = z_engine_core::verification::refresh(report) {
        report.status = TaskStatus::Stale;
        report
            .blockers
            .push(format!("Task evidence could not be revalidated: {error}"));
    } else if previous != TaskStatus::Complete && report.status == TaskStatus::Complete {
        // Refresh is observation only: reopening a transcript cannot complete it.
        report.status = previous;
    }
}

fn interrupt_report(report: &mut TaskReport) {
    report.status = TaskStatus::Interrupted;
    report
        .blockers
        .push("Session stopped before task verification finished.".into());
}

pub(crate) fn sessions_dir() -> PathBuf {
    z_engine_core::config::sessions_dir()
}

pub(crate) fn contain_session(path: &str) -> Result<PathBuf, String> {
    let mut last = "path escapes the session store".to_string();
    for dir in z_engine_core::config::session_search_dirs() {
        match contain(&dir, path) {
            Ok(p) => return Ok(p),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Mark the chat as opened so a done/abort unread dot does not return.
pub(crate) fn ack_session_file(path: &std::path::Path) {
    let events = match z_engine_core::session::read_events(path) {
        Ok(events) => events,
        Err(error) => {
            tracing::warn!(%error, "could not read session to acknowledge it");
            return;
        }
    };
    if z_engine_core::session::unread_outcome(&events).is_none() {
        return;
    }
    if let Ok(mut w) = z_engine_core::session::SessionWriter::append_to(path) {
        if let Err(error) = w.record(&z_engine_core::session::SessionEvent::Ack) {
            tracing::warn!(%error, "could not acknowledge session");
        }
    }
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionEntry {
    pub path: String,
    pub ulid: String,
    pub first_user_msg: Option<String>,
    pub modified_ms: u64,
    pub project_root: Option<String>,
    pub unread_outcome: Option<String>,
}

#[tauri::command]
pub(crate) fn list_sessions(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<SessionEntry>, String> {
    use std::time::UNIX_EPOCH;
    let mut out: Vec<SessionEntry> = Vec::new();
    for dir in z_engine_core::config::session_search_dirs() {
        for s in z_engine_core::session::list_sessions(&dir) {
            let is_live = state.has_loop(&s.ulid)?;
            let unread_outcome = s.unread_outcome.and_then(|_| {
                let events = z_engine_core::session::read_events(&s.path).ok()?;
                projected_unread_outcome(&replayed_events(&events, is_live), is_live)
            });
            out.push(SessionEntry {
                modified_ms: s
                    .modified
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                path: s.path.to_string_lossy().into_owned(),
                ulid: s.ulid,
                first_user_msg: s.first_user_msg,
                project_root: s.project_root,
                unread_outcome,
            });
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.modified_ms));
    Ok(out)
}

#[tauri::command]
pub(crate) fn delete_session(
    path: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let contained = contain_session(&path)?;
    if let Some(ulid) = contained
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
    {
        let _ = state.shutdown_one(&ulid);
    }
    z_engine_core::session::delete_session(&contained).map_err(|e| e.to_string())
}

fn roots_match(session_root: &str, workspace: &std::path::Path) -> bool {
    let a = std::path::PathBuf::from(session_root);
    if a == workspace {
        return true;
    }
    match (std::fs::canonicalize(&a), std::fs::canonicalize(workspace)) {
        (Ok(x), Ok(y)) => x == y,
        _ => {
            let sa = session_root
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_lowercase();
            let sb = workspace
                .to_string_lossy()
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_lowercase();
            sa == sb
        }
    }
}

/// Delete every transcript whose Meta.project_root matches `root`.
pub(crate) fn delete_sessions_for_workspace(
    root: &std::path::Path,
    state: &GuiState,
) -> Result<usize, String> {
    let mut n = 0usize;
    for dir in z_engine_core::config::session_search_dirs() {
        for s in z_engine_core::session::list_sessions(&dir) {
            let Some(pr) = &s.project_root else {
                continue;
            };
            if !roots_match(pr, root) {
                continue;
            }
            let _ = state.shutdown_one(&s.ulid);
            if z_engine_core::session::delete_session(&s.path).is_ok() {
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Transcript replay for the sessions sidebar: parse a session JSONL into
/// its event list so the frontend can rebuild the chat history.
#[tauri::command]
pub(crate) fn read_session(
    path: String,
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<serde_json::Value>, String> {
    let contained = contain_session(&path)?;
    let events = z_engine_core::session::read_events(&contained).map_err(|e| e.to_string())?;
    let id = contained
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    session_events_json(&events, state.has_loop(id)?)
}

#[cfg(test)]
#[path = "session_store_tests.rs"]
mod tests;
