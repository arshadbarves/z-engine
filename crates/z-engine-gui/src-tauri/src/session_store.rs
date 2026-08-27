use crate::git_util::contain;
use std::path::PathBuf;

pub(crate) fn sessions_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("sessions")
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionEntry {
    pub path: String,
    pub ulid: String,
    pub first_user_msg: Option<String>,
    pub modified_ms: u64,
    pub project_root: Option<String>,
}

#[tauri::command]
pub(crate) fn list_sessions() -> Result<Vec<SessionEntry>, String> {
    use std::time::UNIX_EPOCH;
    Ok(harness_core::session::list_sessions(&sessions_dir())
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
        })
        .collect())
}

#[tauri::command]
pub(crate) fn delete_session(path: String) -> Result<(), String> {
    let contained = contain(&sessions_dir(), &path)?;
    harness_core::session::delete_session(&contained).map_err(|e| e.to_string())
}

/// Transcript replay for the sessions sidebar: parse a session JSONL into
/// its event list so the frontend can rebuild the chat history.
#[tauri::command]
pub(crate) fn read_session(path: String) -> Result<Vec<serde_json::Value>, String> {
    let events = harness_core::session::read_events(std::path::Path::new(&path))
        .map_err(|e| e.to_string())?;
    Ok(events
        .into_iter()
        .map(|e| serde_json::to_value(&e).unwrap_or(serde_json::Value::Null))
        .collect())
}
