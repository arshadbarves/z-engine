//! Truncate a session JSONL to the events before a given user-turn index.
//! Used by per-message revert so reopening a session does not resurrect
//! the dropped turns.

use std::path::Path;

use super::SessionEvent;
use super::reader::read_transcript;
use super::storage::{lock, path_state, replace_events};

/// Keep every event up to (not including) the `keep`-th `UserMsg`
/// (0-based). `keep == 0` retains only the prefix before the first user
/// message (typically `Meta`). If `keep` is past the last user message,
/// the input is returned unchanged.
pub fn events_before_user_turn(events: &[SessionEvent], keep: u64) -> Vec<SessionEvent> {
    let mut seen = 0u64;
    let mut cut = events.len();
    for (i, ev) in events.iter().enumerate() {
        if matches!(ev, SessionEvent::UserMsg { .. }) {
            if seen == keep {
                cut = i;
                break;
            }
            seen += 1;
        }
    }
    events[..cut].to_vec()
}

/// Rewrite `path` so it only contains events before user-turn `keep`.
/// Existing append handles automatically follow the replacement.
pub fn trim_file_before_user_turn(path: &Path, keep: u64) -> std::io::Result<()> {
    let shared = path_state(path)?;
    let mut state = lock(&shared)?;
    state.ensure_healthy()?;
    let events = read_transcript(path)?.events;
    let kept = events_before_user_turn(&events, keep);
    replace_events(path, &kept, &mut state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionEvent, SessionWriter, read_events};

    fn user(text: &str) -> SessionEvent {
        SessionEvent::UserMsg {
            text: text.into(),
            images: vec![],
        }
    }

    fn meta() -> SessionEvent {
        SessionEvent::Meta {
            model: "m".into(),
            project_root: "/tmp".into(),
        }
    }

    fn assistant(text: &str) -> SessionEvent {
        SessionEvent::AssistantMsg {
            content: Some(text.into()),
            tool_calls: vec![],
        }
    }

    #[test]
    fn keep_zero_drops_from_first_user_msg() {
        let events = vec![meta(), user("one"), assistant("ok"), user("two")];
        let kept = events_before_user_turn(&events, 0);
        assert_eq!(kept, vec![meta()]);
    }

    #[test]
    fn keep_one_retains_first_turn_only() {
        let events = vec![
            meta(),
            user("one"),
            assistant("a"),
            user("two"),
            assistant("b"),
        ];
        let kept = events_before_user_turn(&events, 1);
        assert_eq!(kept, vec![meta(), user("one"), assistant("a")]);
    }

    #[test]
    fn keep_past_end_is_noop() {
        let events = vec![meta(), user("one")];
        assert_eq!(events_before_user_turn(&events, 5), events);
    }

    #[test]
    fn rewrite_drops_later_turns_from_disk() {
        let dir = tempfile::tempdir_in(".").unwrap();
        let mut w = SessionWriter::create(dir.path()).unwrap();
        for ev in [meta(), user("one"), assistant("a"), user("two")] {
            w.record(&ev).unwrap();
        }
        let path = w.path.clone();
        trim_file_before_user_turn(&path, 1).unwrap();
        let read = read_events(&path).unwrap();
        assert_eq!(read, vec![meta(), user("one"), assistant("a")]);
    }
}
