//! Session persistence (spec §8): append-only JSONL transcripts at
//! `$XDG_DATA_HOME|~/Library/Application Support/z-engine/sessions/<ulid>.jsonl`.
//!
//! Crash-safe by construction: the writer only ever appends whole lines and
//! flushes per event; a `kill -9` can at worst truncate the final line,
//! which readers skip. Resume replays events into a fresh loop state.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod title;
mod trim;
pub use title::{display_title, fallback_title};
pub use trim::{events_before_user_turn, trim_file_before_user_turn};

/// One persisted transcript event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// The initial environment description for a session.
    Meta { model: String, project_root: String },
    UserMsg {
        text: String,
        /// Attached images as data URLs (vision input); usually empty.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
    },
    /// One assistant turn: prose plus any tool calls it emitted.
    AssistantMsg {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<PersistedToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    /// Compaction summaries and similar durable annotations (L1).
    Note { text: String },
    /// Short display title for the session (Codex/Claude-style).
    Title { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Append handle for one session file.
#[derive(Debug)]
pub struct SessionWriter {
    file: File,
    pub path: PathBuf,
}

impl SessionWriter {
    /// Create a brand-new session file named by a fresh ULID.
    pub fn create(sessions_dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(sessions_dir)?;
        let path = sessions_dir.join(format!("{}.jsonl", ulid::Ulid::new()));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { file, path })
    }

    /// Append to an existing session (resume path).
    pub fn append_to(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Append one event as a single flushed line.
    pub fn record(&mut self, event: &SessionEvent) -> std::io::Result<()> {
        let line = serde_json::to_string(event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.flush()
    }

    /// Re-open the append handle after the file was replaced (e.g. trim).
    pub fn reopen(&mut self) -> std::io::Result<()> {
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        Ok(())
    }
}

/// Parse one JSONL line; malformed lines are logged and skipped so a torn
/// trailing write after a crash never aborts replay.
fn parse_line(line: &str) -> Option<SessionEvent> {
    match serde_json::from_str(line) {
        Ok(ev) => Some(ev),
        Err(e) => {
            tracing::warn!(error = %e, len = line.len(), "skipping bad session line");
            None
        }
    }
}

/// Delete a session file (used by the GUI sessions manager).
pub fn delete_session(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

/// Read all well-formed events from a session file.
pub fn read_events(path: &Path) -> std::io::Result<Vec<SessionEvent>> {
    let f = File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(ev) = parse_line(&line) {
            out.push(ev);
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub path: PathBuf,
    pub ulid: String,
    /// Sidebar label: persisted `Title` event, else a short first-line fallback.
    pub first_user_msg: Option<String>,
    pub modified: std::time::SystemTime,
    /// Project root recorded in the session's `Meta` event — lets the GUI
    /// group transcripts under their workspace.
    pub project_root: Option<String>,
}

/// List sessions under `sessions_dir`, newest first.
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
        });
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.modified));
    out
}

/// Working state rebuilt from persisted events.
#[derive(Debug)]
pub struct Replayed {
    pub working: Vec<z_engine_provider::ChatMessage>,
    /// Note texts + recorded `update_context_notes` arguments (for L1).
    pub notes_replayed: Vec<String>,
}

/// Rebuild working-set messages + durable note texts from events.
pub fn replay(events: &[SessionEvent]) -> Replayed {
    use z_engine_provider::{ChatMessage, FunctionCall, ToolCall};

    let mut working: Vec<ChatMessage> = Vec::new();
    let mut notes_replayed = Vec::new();
    // Assistant rounds whose tool results may not have landed (crash).
    let mut pending_rounds: Vec<usize> = Vec::new();

    for ev in events {
        match ev {
            SessionEvent::Meta { .. } => {}
            SessionEvent::UserMsg { text, images } => {
                working.push(ChatMessage::user_with_images(text.clone(), images));
            }
            SessionEvent::AssistantMsg {
                content,
                tool_calls,
            } => {
                let converted: Vec<ToolCall> = tool_calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        function: FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect();
                if !tool_calls.is_empty() {
                    pending_rounds.push(working.len());
                }
                for tc in tool_calls {
                    // Notes protocol: re-feed recorded meta-output verbatim.
                    if tc.name == "update_context_notes" {
                        notes_replayed.push(tc.arguments.clone());
                    }
                }
                working.push(ChatMessage::Assistant {
                    content: content.clone(),
                    tool_calls: converted,
                });
            }
            SessionEvent::ToolResult {
                tool_call_id,
                content,
            } => {
                // A landed result closes the most recent pending round.
                pending_rounds.pop();
                working.push(ChatMessage::tool_result(
                    tool_call_id.clone(),
                    content.clone(),
                ));
            }
            SessionEvent::Note { text } => notes_replayed.push(text.clone()),
            SessionEvent::Title { .. } => {}
        }
    }

    // Trim trailing rounds whose results never arrived (crash mid-round):
    // providers reject assistant tool-calls without their replies.
    while let Some(&idx) = pending_rounds.last() {
        working.truncate(idx);
        pending_rounds.pop();
    }

    Replayed {
        working,
        notes_replayed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_events() -> Vec<SessionEvent> {
        vec![
            SessionEvent::Meta {
                model: "m".into(),
                project_root: "/tmp".into(),
            },
            SessionEvent::UserMsg {
                text: "fix it".into(),
                images: vec![],
            },
            SessionEvent::AssistantMsg {
                content: Some("looking".into()),
                tool_calls: vec![PersistedToolCall {
                    id: "c1".into(),
                    name: "read_file".into(),
                    arguments: r#"{"path":"a.txt"}"#.into(),
                }],
            },
            SessionEvent::ToolResult {
                tool_call_id: "c1".into(),
                content: "contents".into(),
            },
            SessionEvent::Note {
                text: "FACTS: something".into(),
            },
        ]
    }

    #[test]
    fn roundtrip_preserves_events() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = SessionWriter::create(dir.path()).unwrap();
        for ev in sample_events() {
            w.record(&ev).unwrap();
        }
        let read = read_events(&w.path).unwrap();
        assert_eq!(read, sample_events());
    }

    #[test]
    fn torn_final_line_is_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = SessionWriter::create(dir.path()).unwrap();
        w.record(&sample_events()[1]).unwrap();
        use std::io::Write;
        w.file
            .write_all(b"{\"type\":\"user_msg\",\"text\":\"trunca")
            .expect("simulate torn write");
        w.file.flush().unwrap();

        let read = read_events(&w.path).unwrap();
        assert_eq!(read.len(), 1);
        assert!(matches!(read[0], SessionEvent::UserMsg { .. }));
    }

    #[test]
    fn replay_maps_to_chat_messages_and_notes() {
        let r = replay(&sample_events());
        assert_eq!(r.working.len(), 3);
        assert!(matches!(
            &r.working[1],
            z_engine_provider::ChatMessage::Assistant { tool_calls, .. } if tool_calls.len() == 1
        ));
        assert!(r.notes_replayed.contains(&"FACTS: something".to_string()));
    }

    #[test]
    fn replay_drops_trailing_orphaned_tool_round() {
        let events = vec![
            SessionEvent::UserMsg {
                text: "go".into(),
                images: vec![],
            },
            SessionEvent::AssistantMsg {
                content: None,
                tool_calls: vec![PersistedToolCall {
                    id: "c9".into(),
                    name: "bash".into(),
                    arguments: "{}".into(),
                }],
            },
            // crash before ToolResult landed
        ];
        let r = replay(&events);
        assert_eq!(r.working.len(), 1);
        assert!(matches!(
            &r.working[0],
            z_engine_provider::ChatMessage::User { .. }
        ));
    }

    #[test]
    fn list_sessions_orders_newest_first_and_previews() {
        let dir = tempfile::tempdir().unwrap();
        let mut older = SessionWriter::create(dir.path()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut newer = SessionWriter::create(dir.path()).unwrap();
        older
            .record(&SessionEvent::UserMsg {
                text: "old task".into(),
                images: vec![],
            })
            .unwrap();
        newer
            .record(&SessionEvent::UserMsg {
                text: "new task".into(),
                images: vec![],
            })
            .unwrap();

        let list = list_sessions(dir.path());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].first_user_msg.as_deref(), Some("new task"));
        assert_eq!(list[1].first_user_msg.as_deref(), Some("old task"));
    }
}
