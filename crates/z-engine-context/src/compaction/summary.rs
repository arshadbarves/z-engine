//! Summary compaction: choose where the verbatim tail starts, then replace
//! everything before it with one summary message.

use std::collections::HashMap;

use z_engine_prompts::reminders::COMPACTION_SUMMARY;
use z_engine_protocol::{CallId, ContentBlock, Message, Role};

use crate::template::render_template;

/// Where summary compaction cuts the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryPlan {
    /// `messages[..split]` is summarized; `messages[split..]` stays verbatim.
    pub split: usize,
}

/// The latest split that keeps at least `keep_recent_messages` messages
/// verbatim and lands on the start of a real user turn: a user message
/// holding no tool results (tool-round messages may also carry steering or
/// reminder text, so "not tool-results-only" is not enough). A split never
/// separates a tool_use from its tool_result. `None` when no split leaves
/// anything to summarize.
pub fn plan_summary(messages: &[Message], keep_recent_messages: usize) -> Option<SummaryPlan> {
    let latest = messages
        .len()
        .checked_sub(keep_recent_messages)?
        .min(messages.len().saturating_sub(1));
    let crossed = crossed_splits(messages);
    (1..=latest)
        .rev()
        .find(|&split| !crossed[split] && starts_user_turn(&messages[split]))
        .map(|split| SummaryPlan { split })
}

/// A user message whose text is [`COMPACTION_SUMMARY`] around `summary`.
pub fn summary_message(summary: &str) -> Message {
    let text = render_template(COMPACTION_SUMMARY, &[("summary", summary.trim())]);
    Message::user_text(text.trim_end())
}

/// `summary` followed by the verbatim tail `messages[plan.split..]`.
/// `plan` must come from [`plan_summary`] on the same messages; a split past
/// the end keeps no tail.
pub fn apply_summary(messages: &[Message], plan: &SummaryPlan, summary: &Message) -> Vec<Message> {
    let tail = &messages[plan.split.min(messages.len())..];
    let mut out = Vec::with_capacity(tail.len() + 1);
    out.push(summary.clone());
    out.extend_from_slice(tail);
    out
}

fn starts_user_turn(message: &Message) -> bool {
    message.role == Role::User
        && !message.content.is_empty()
        && !message
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
}

/// `crossed[s]` is true when a tool_use and its tool_result sit on
/// opposite sides of split `s` (one of them before index `s`, the other at
/// or after it). Has `messages.len() + 1` entries.
fn crossed_splits(messages: &[Message]) -> Vec<bool> {
    let mut uses: HashMap<&CallId, usize> = HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        for (id, _, _) in message.tool_uses() {
            uses.entry(id).or_insert(index);
        }
    }
    // Difference array over split positions: a pair at (lo, hi) crosses
    // every split in lo + 1..=hi.
    let mut delta = vec![0i64; messages.len() + 2];
    for (index, message) in messages.iter().enumerate() {
        for block in &message.content {
            let ContentBlock::ToolResult { tool_use_id, .. } = block else {
                continue;
            };
            let Some(&use_index) = uses.get(tool_use_id) else {
                continue;
            };
            let (lo, hi) = (use_index.min(index), use_index.max(index));
            if lo < hi {
                delta[lo + 1] += 1;
                delta[hi + 1] -= 1;
            }
        }
    }
    let mut open = 0i64;
    delta[..=messages.len()]
        .iter()
        .map(|change| {
            open += change;
            open > 0
        })
        .collect()
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
