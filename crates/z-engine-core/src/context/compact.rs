//! Transactional compaction planning: spill tool output before replacing it,
//! and propose prose demotion only for whole messages sent to the summarizer.
//! The caller commits a plan only after durably recording a nonempty summary.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use z_engine_provider::ChatMessage;

/// How many trailing messages stay verbatim (L2 window).
pub const DEFAULT_KEEP_RECENT: usize = 12;
/// Must not exceed the input ceiling of `agent::side_requests::summarize_segment`.
pub(crate) const MAX_SUMMARIZE_CHARS: usize = 12_000;
const SPILL_MARKER_PREFIX: &str = "[harness:elided; full: ";

#[derive(Debug, thiserror::Error)]
pub enum CompactionError {
    #[error("context spill I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("context spill path is not UTF-8: {0:?}")]
    InvalidSpillPath(PathBuf),
}

impl CompactionError {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

#[derive(Debug, Default)]
pub struct CompactionOutcome {
    /// Candidate messages; not safe to install until summarization is committed.
    pub messages: Vec<ChatMessage>,
    /// Concatenated old-turn prose awaiting summarization (L3 → L1).
    pub summarize_input: String,
    pub elided_tool_outputs: usize,
    pub dropped_prose_messages: usize,
}

fn spill(content: &str, tmp_dir: &Path) -> Result<PathBuf, CompactionError> {
    let dir = tmp_dir.join("z-engine");
    std::fs::create_dir_all(&dir).map_err(|error| CompactionError::io(&dir, error))?;
    let path = dir.join(format!("ctx-{}.log", ulid::Ulid::new()));
    if path.to_str().is_none() {
        return Err(CompactionError::InvalidSpillPath(path));
    }
    let mut file =
        std::fs::File::create_new(&path).map_err(|error| CompactionError::io(&path, error))?;
    if let Err(source) = file
        .write_all(content.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        if let Err(error) = std::fs::remove_file(&path) {
            tracing::warn!(path = %path.display(), %error, "failed removing incomplete context spill");
        }
        return Err(CompactionError::io(&path, source));
    }
    #[cfg(unix)]
    std::fs::File::open(&dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| CompactionError::io(&dir, error))?;
    Ok(path)
}

fn existing_spill_path(content: &str) -> Option<PathBuf> {
    let body = if content.starts_with("[harness:tool-output id=") {
        let (id, body) = content.split_once('\n')?;
        if !id.ends_with(']') {
            return None;
        }
        body
    } else {
        content
    };
    let path = body.strip_prefix(SPILL_MARKER_PREFIX)?.strip_suffix(']')?;
    Some(PathBuf::from(path))
}

fn elide_text(content: &str, tmp_dir: &Path) -> Result<Option<String>, CompactionError> {
    if let Some(path) = existing_spill_path(content) {
        let file = std::fs::File::open(&path).map_err(|error| CompactionError::io(&path, error))?;
        let metadata = file
            .metadata()
            .map_err(|error| CompactionError::io(&path, error))?;
        if !metadata.is_file() {
            return Err(CompactionError::io(
                &path,
                std::io::Error::other("context spill is not a regular file"),
            ));
        }
        return Ok(None);
    }
    // Preserve the harness tool-output id marker if present so later
    // droppable references still resolve.
    let id_line = content
        .lines()
        .find(|l| l.starts_with("[harness:tool-output id="))
        .map(|l| format!("{l}\n"))
        .unwrap_or_default();
    let path = spill(content, tmp_dir)?;
    Ok(Some(format!(
        "{id_line}[harness:elided; full: {}]",
        path.display()
    )))
}

/// Replace tool-result contents referenced by `droppable_ids` anywhere in
/// the list. No messages are changed unless every required spill succeeds.
pub fn elide_droppable(
    messages: &mut [ChatMessage],
    droppable_ids: &BTreeSet<String>,
    tmp_dir: &Path,
) -> Result<usize, CompactionError> {
    if droppable_ids.is_empty() {
        return Ok(0);
    }
    let mut replacements = Vec::new();
    for (index, msg) in messages.iter().enumerate() {
        if let ChatMessage::Tool { content, .. } = msg {
            for id in droppable_ids {
                if content.contains(&format!("[harness:tool-output id={id}]")) {
                    if let Some(elided) = elide_text(content, tmp_dir)? {
                        replacements.push((index, elided));
                    }
                    break;
                }
            }
        }
    }
    let count = replacements.len();
    for (index, elided) in replacements {
        if let ChatMessage::Tool { content, .. } = &mut messages[index] {
            *content = elided;
        }
    }
    Ok(count)
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
pub fn compact(
    messages: &[ChatMessage],
    keep_recent: usize,
    tmp_dir: &Path,
) -> Result<CompactionOutcome, CompactionError> {
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
                if let Some(elided) = elide_text(content, tmp_dir)? {
                    outcome.elided_tool_outputs += 1;
                    out.push(ChatMessage::Tool {
                        tool_call_id: tool_call_id.clone(),
                        content: elided,
                    });
                } else {
                    out.push(msg.clone());
                }
            }
            ChatMessage::User { content } if queue_prose(&mut outcome, "[user]", content) => {}
            ChatMessage::Assistant {
                content: Some(text),
                tool_calls,
            } if tool_calls.is_empty() && queue_prose(&mut outcome, "[assistant]", text) => {}
            // Keep images, tool-call narration, and prose not sent to the summarizer.
            _ => out.push(msg.clone()),
        }
    }

    outcome.messages = out;
    Ok(outcome)
}

fn queue_prose(outcome: &mut CompactionOutcome, role: &str, text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let part = format!("{role} {text}");
    let separator = usize::from(!outcome.summarize_input.is_empty());
    if outcome.summarize_input.chars().count() + separator + part.chars().count()
        > MAX_SUMMARIZE_CHARS
    {
        return false;
    }
    if separator > 0 {
        outcome.summarize_input.push('\n');
    }
    outcome.summarize_input.push_str(&part);
    outcome.dropped_prose_messages += 1;
    true
}

#[cfg(test)]
#[path = "compact_spill_tests.rs"]
mod spill_tests;
#[cfg(test)]
#[path = "compact_tests.rs"]
mod tests;
