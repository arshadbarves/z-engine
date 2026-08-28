use crate::git_util::contain;
use crate::state::GuiState;
use std::path::PathBuf;
use z_engine_core::session::SessionEvent;

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartSessionResult {
    pub ulid: String,
    pub events: Vec<serde_json::Value>,
    pub already_live: bool,
    pub path: Option<String>,
}

pub(crate) fn session_events_json(events: &[SessionEvent]) -> Vec<serde_json::Value> {
    events
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
        .collect()
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
    let events = z_engine_core::session::read_events(path).unwrap_or_default();
    if z_engine_core::session::unread_outcome(&events).is_none() {
        return;
    }
    if let Ok(mut w) = z_engine_core::session::SessionWriter::append_to(path) {
        let _ = w.record(&z_engine_core::session::SessionEvent::Ack);
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
pub(crate) fn list_sessions() -> Result<Vec<SessionEntry>, String> {
    use std::time::UNIX_EPOCH;
    let mut out: Vec<SessionEntry> = Vec::new();
    for dir in z_engine_core::config::session_search_dirs() {
        out.extend(
            z_engine_core::session::list_sessions(&dir)
                .into_iter()
                .map(|s| SessionEntry {
                    modified_ms: s
                        .modified
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                    path: s.path.to_string_lossy().into_owned(),
                    ulid: s.ulid,
                    first_user_msg: s.first_user_msg,
                    project_root: s.project_root,
                    unread_outcome: s.unread_outcome,
                }),
        );
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
pub(crate) fn read_session(path: String) -> Result<Vec<serde_json::Value>, String> {
    let contained = contain_session(&path)?;
    let events = z_engine_core::session::read_events(&contained).map_err(|e| e.to_string())?;
    Ok(session_events_json(&events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn roots_match_ignores_slash_and_case() {
        assert!(roots_match("/tmp/proj", Path::new("/tmp/proj/")));
        assert!(roots_match("/tmp/Proj", Path::new("/tmp/proj")));
        assert!(!roots_match("/tmp/proj", Path::new("/tmp/other")));
    }
}
