//! Compaction engine (spec §6): token-budget-driven progressive demotion.
//!
//! Pure planning/transformation over the message list — the agent loop
//! supplies provider access for the summarization side-request separately.
//!
//! Ladder behavior implemented here:
//! - **L4** — every tool result older than the recent-tail window is
//!   replaced by `[elided; full: <spill file>]`; the complete text lands in
//!   a temp file first;
//! - **L3** — old user/assistant prose is extracted and returned so the
//!   caller can summarize it into L1 notes (the messages themselves are
//!   dropped);
//! - tool rounds are kept structurally intact (an assistant tool-call is
//!   never separated from its replies — providers reject orphans);
//! - **eager droppable** — entries the model flagged with a
//!   `[harness:tool-output id=…]` reference are elided immediately,
//!   regardless of position.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use z_engine_provider::{ChatMessage, ContentPart};

/// How many trailing messages stay verbatim (L2 window).
pub const DEFAULT_KEEP_RECENT: usize = 12;
/// Tool outputs larger than this get spilled even inside the tail when the
/// model marks them droppable (handled elsewhere) — here purely informational.
const SPILL_MARKER_PREFIX: &str = "[harness:elided; full: ";

#[derive(Debug, Default)]
pub struct CompactionOutcome {
    /// Transformed message list (same length unless L3 prose was dropped).
    pub messages: Vec<ChatMessage>,
    /// Concatenated old-turn prose awaiting summarization (L3 → L1).
    pub summarize_input: String,
    pub elided_tool_outputs: usize,
    pub dropped_prose_messages: usize,
}

fn spill(content: &str, tmp_dir: &Path) -> PathBuf {
    let dir = tmp_dir.join("z-engine");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("ctx-{}.log", ulid::Ulid::new()));
    if std::fs::write(&path, content).is_err() {
        return PathBuf::from("<unavailable>");
    }
    path
}

fn existing_spill_path(content: &str) -> Option<PathBuf> {
    let idx = content.find(SPILL_MARKER_PREFIX)?;
    let rest = &content[idx + SPILL_MARKER_PREFIX.len()..];
    let end = rest.find(']')?;
    Some(PathBuf::from(rest[..end].to_string()))
}

fn elide_text(content: &str, tmp_dir: &Path) -> String {
    // Preserve the harness tool-output id marker if present so later
    // droppable references still resolve.
    let id_line = content
        .lines()
        .find(|l| l.starts_with("[harness:tool-output id="))
        .map(|l| format!("{l}\n"))
        .unwrap_or_default();
    let path = existing_spill_path(content).unwrap_or_else(|| spill(content, tmp_dir));
    format!("{id_line}[harness:elided; full: {}]", path.display())
}

/// Replace tool-result contents referenced by `droppable_ids` anywhere in
/// the list. Returns how many were elided.
pub fn elide_droppable(
    messages: &mut [ChatMessage],
    droppable_ids: &BTreeSet<String>,
    tmp_dir: &Path,
) -> usize {
    if droppable_ids.is_empty() {
        return 0;
    }
    let mut count = 0;
    for msg in messages.iter_mut() {
        if let ChatMessage::Tool { content, .. } = msg {
            for id in droppable_ids {
                if content.contains(&format!("[harness:tool-output id={id}]")) {
                    *content = elide_text(content, tmp_dir);
                    count += 1;
                    break;
                }
            }
        }
    }
    count
}

/// True when this assistant message carries tool calls (round anchor).
fn is_assistant_round(msg: &ChatMessage) -> bool {
    matches!(msg, ChatMessage::Assistant { tool_calls, .. } if !tool_calls.is_empty())
}

/// Index where the verbatim tail must start so it never begins mid-round
/// (i.e., never splits an assistant tool-call from its tool replies).
pub fn round_safe_tail_start(messages_len: usize, keep_recent: usize) -> usize {
    // Caller adjusts with actual messages; here we only clamp.
    messages_len.saturating_sub(keep_recent)
}

/// Compact the working set: elide L4, extract L3 prose for summarization.
///
/// `messages` excludes the L0 system prompt and L1 notes block (those are
/// re-injected by the caller and never modified).
pub fn compact(messages: &[ChatMessage], keep_recent: usize, tmp_dir: &Path) -> CompactionOutcome {
    let mut out = Vec::with_capacity(messages.len());
    let mut outcome = CompactionOutcome::default();

    let raw_tail_start = round_safe_tail_start(messages.len(), keep_recent);
    // Walk back off orphaned tool replies / their anchors.
    let mut tail_start = raw_tail_start.min(messages.len());
    while tail_start > 0 && matches!(messages.get(tail_start), Some(ChatMessage::Tool { .. })) {
        tail_start -= 1;
    }
    // Don't leave a trailing assistant-with-calls in the head without its
    // replies: extend head to include the whole round instead.
    while tail_start < messages.len()
        && is_assistant_round(&messages[tail_start])
        && tail_start + 1 < messages.len()
        && messages[tail_start + 1..]
            .iter()
            .take_while(|m| matches!(m, ChatMessage::Tool { .. }))
            .count()
            == 0
    {
        tail_start += 1;
    }

    let mut summary_parts: Vec<String> = Vec::new();

    for (idx, msg) in messages.iter().enumerate() {
        if idx >= tail_start {
            out.push(msg.clone());
            continue;
        }
        match msg {
            ChatMessage::Tool {
                tool_call_id,
                content,
            } => {
                // L4: elide, preserving structure.
                let elided = elide_text(content, tmp_dir);
                outcome.elided_tool_outputs += 1;
                out.push(ChatMessage::Tool {
                    tool_call_id: tool_call_id.clone(),
                    content: elided,
                });
            }
            ChatMessage::User { content } => {
                summary_parts.push(format!("[user] {content}"));
                outcome.dropped_prose_messages += 1;
            }
            ChatMessage::UserMulti { content } => {
                // Vision turns are rare and expensive to re-create —
                // summarize the text parts, drop the image payloads.
                let texts: Vec<&str> = content
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                summary_parts.push(format!("[user] {}", texts.join(" ")));
                outcome.dropped_prose_messages += 1;
            }
            ChatMessage::Assistant {
                content,
                tool_calls,
            } => {
                if tool_calls.is_empty() {
                    if let Some(text) = content {
                        summary_parts.push(format!("[assistant] {text}"));
                        outcome.dropped_prose_messages += 1;
                    }
                } else {
                    // Round anchor in head: keep the call, elide prose.
                    let elided_content = content
                        .as_ref()
                        .map(|_| "[earlier narration elided]".to_string());
                    out.push(ChatMessage::Assistant {
                        content: elided_content,
                        tool_calls: tool_calls.clone(),
                    });
                }
            }
            ChatMessage::System { content } => {
                // Shouldn't appear in the working set; preserve defensively.
                let _ = content;
                out.push(msg.clone());
            }
        }
    }

    if !summary_parts.is_empty() {
        outcome.summarize_input = summary_parts.join("\n");
    }
    outcome.messages = out;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use z_engine_provider::{FunctionCall, ToolCall};

    fn tool_msg(id: &str, content: &str) -> ChatMessage {
        ChatMessage::Tool {
            tool_call_id: id.into(),
            content: content.into(),
        }
    }

    fn assistant_with_tools(id: &str) -> ChatMessage {
        ChatMessage::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: id.into(),
                function: FunctionCall {
                    name: "read_file".into(),
                    arguments: "{}".into(),
                },
            }],
        }
    }

    fn sample() -> Vec<ChatMessage> {
        vec![
            ChatMessage::user("early task"),
            ChatMessage::assistant_text("early answer"),
            assistant_with_tools("c1"),
            tool_msg("c1", "big output A"),
            ChatMessage::user("mid task"),
            ChatMessage::assistant_text("mid answer"),
            assistant_with_tools("c2"),
            tool_msg("c2", "big output B"),
            ChatMessage::user("recent task"),
            ChatMessage::assistant_text("recent answer"),
        ]
    }

    #[test]
    fn tail_stays_verbatim_and_head_elides() {
        let tmp = tempfile::tempdir().unwrap();
        let msgs = sample();
        let out = compact(&msgs, 4, tmp.path());

        // Head prose is dropped (lives only in summarize_input); tool rounds
        // survive structurally with elided contents; tail stays verbatim:
        // [round c1, elided c1, round c2, elided c2, recent user, recent asst]
        assert_eq!(out.messages.len(), 6);
        assert!(
            matches!(&out.messages[0], ChatMessage::Assistant { tool_calls, .. } if !tool_calls.is_empty())
        );

        let elided_c1 = match &out.messages[1] {
            ChatMessage::Tool { content, .. } => content.clone(),
            other => panic!("{other:?}"),
        };
        assert!(
            elided_c1.starts_with("[harness:elided; full: "),
            "{elided_c1}"
        );
        let path = existing_spill_path(&elided_c1).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "big output A");

        // c2's round lies INSIDE the verbatim tail (raw start hit its
        // anchor) — its output must be untouched.
        assert!(
            matches!(&out.messages[3], ChatMessage::Tool { content, .. } if content == "big output B")
        );
        assert!(
            matches!(&out.messages[4], ChatMessage::User { content } if content == "recent task")
        );
        assert!(matches!(&out.messages[5], ChatMessage::Assistant { .. }));

        assert!(out.summarize_input.contains("[user] early task"));
        assert!(out.summarize_input.contains("[assistant] early answer"));
        assert!(out.summarize_input.contains("[user] mid task"));
        assert!(out.summarize_input.contains("[assistant] mid answer"));
        assert!(!out.summarize_input.contains("recent task"));
        assert_eq!(out.elided_tool_outputs, 1);
        assert_eq!(out.dropped_prose_messages, 4);
    }

    #[test]
    fn tail_never_starts_on_orphaned_tool_reply() {
        let msgs = vec![
            ChatMessage::user("u1"),
            assistant_with_tools("x"),
            tool_msg("x", "out"),
            ChatMessage::user("u2"),
        ];
        // keep_recent=2 would naively start at index 2 (a Tool) — walk-back
        // must land on the assistant at index 1 so the round stays whole;
        // the leading prose user msg is demoted to the summarizer.
        let tmp = tempfile::tempdir().unwrap();
        let out = compact(&msgs, 2, tmp.path());
        assert_eq!(out.messages.len(), 3);
        assert!(
            matches!(out.messages.first(), Some(ChatMessage::Assistant { tool_calls, .. }) if !tool_calls.is_empty())
        );
        assert!(
            matches!(out.messages.get(2), Some(ChatMessage::User { content }) if content == "u2")
        );
    }

    #[test]
    fn elide_droppable_hits_anywhere_by_id() {
        let tmp = tempfile::tempdir().unwrap();
        let mut msgs = vec![
            tool_msg("k", "[harness:tool-output id=abcd1234]\nlong thing\n"),
            ChatMessage::user("later"),
            tool_msg("m", "fresh output"),
        ];
        let mut ids = BTreeSet::new();
        ids.insert("abcd1234".to_string());
        let n = elide_droppable(&mut msgs, &ids, tmp.path());
        assert_eq!(n, 1);
        match &msgs[0] {
            ChatMessage::Tool { content, .. } => {
                assert!(content.contains("[harness:elided"), "{content}");
                assert!(content.contains("id=abcd1234"), "id marker preserved");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(&msgs[2], ChatMessage::Tool { content, .. } if content == "fresh output"));
    }

    #[test]
    fn small_list_is_untouched() {
        let msgs = sample();
        let tmp = tempfile::tempdir().unwrap();
        let out = compact(&msgs, 50, tmp.path());
        // Everything fits in the tail: nothing demoted.
        assert_eq!(out.messages.len(), msgs.len());
        assert_eq!(out.elided_tool_outputs, 0);
        assert!(out.summarize_input.is_empty());
    }
}
