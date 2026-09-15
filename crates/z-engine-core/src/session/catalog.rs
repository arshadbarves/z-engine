use std::path::Path;

use super::storage::{lock, path_state};
use super::{SessionEvent, SessionSummary, display_title, read_events, unread_outcome};

pub fn delete_session(path: &Path) -> std::io::Result<()> {
    let shared = path_state(path)?;
    let mut state = lock(&shared)?;
    std::fs::remove_file(path)?;
    state
        .remember::<()>(Err(std::io::Error::other("session was deleted")))
        .ok();
    Ok(())
}

/// List sessions newest first; unread status is absent for unreadable transcripts.
pub fn list_sessions(sessions_dir: &Path) -> Vec<SessionSummary> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(sessions_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let ulid = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let events = read_events(&path).ok();
        let first_user_msg = events.as_deref().and_then(display_title);
        let unread_outcome = events.as_deref().and_then(unread_outcome);
        let project_root = events.as_deref().and_then(|events| {
            events.iter().find_map(|ev| match ev {
                SessionEvent::Meta { project_root, .. } => Some(project_root.clone()),
                _ => None,
            })
        });
        out.push(SessionSummary {
            path,
            ulid,
            first_user_msg: first_user_msg.map(|t| t.chars().take(80).collect()),
            modified,
            project_root,
            unread_outcome,
        });
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.modified));
    out
}
