//! Per-conversation mutable state threaded through the turn pipeline.

use harness_provider::{ChatMessage, ContentPart, Usage};

pub(super) struct LoopState {
    /// Everything between the L0/L1 prefix and the current turn.
    pub(super) working: Vec<ChatMessage>,
    pub(super) approval_counter: u64,
    /// Last provider-reported usage (authoritative pressure signal).
    pub(super) last_usage: Usage,
    /// Set by Command::Compact.
    pub(super) force_compact: bool,
    /// Rendered repository symbol map, regenerated when dirty.
    pub(super) repo_map_text: Option<String>,
    /// The active task text (reviewer prompt context).
    pub(super) current_task: String,
    /// Reasoning effort for reasoning-capable models; `None` = omit param.
    pub(super) reasoning_effort: Option<String>,
}

impl LoopState {
    pub(super) fn estimate_working(&self) -> u64 {
        let mut bytes = 0usize;
        for m in &self.working {
            let text = match m {
                ChatMessage::System { content }
                | ChatMessage::User { content }
                | ChatMessage::Tool { content, .. } => content.as_str(),
                ChatMessage::UserMulti { content } => {
                    let mut n = 0usize;
                    for part in content {
                        if let ContentPart::Text { text } = part {
                            n += text.len();
                        }
                        if let ContentPart::ImageUrl { image_url } = part {
                            // Rough vision-token proxy: data URLs are big.
                            n += image_url.url.len() / 4;
                        }
                    }
                    // handled below via push
                    bytes += n;
                    continue;
                }
                ChatMessage::Assistant { content, .. } => content.as_deref().unwrap_or(""),
            };
            bytes += text.len();
        }
        // ~4 bytes per token for code/English; estimator calibrated in v1.0.
        (bytes as u64 / 4).max(if bytes > 0 { 1 } else { 0 })
    }

    pub(super) fn pressure_tokens(&self) -> u64 {
        // Provider-reported usage is authoritative once available; before
        // that, fall back to the local estimator.
        let reported = self.last_usage.prompt_tokens + self.last_usage.completion_tokens;
        if reported > 0 {
            reported
        } else {
            self.estimate_working()
        }
    }
}
