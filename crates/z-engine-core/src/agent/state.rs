//! Per-conversation mutable state threaded through the turn pipeline.

use std::sync::{Arc, Mutex};

use z_engine_provider::{ChatMessage, ContentPart, Usage};

use super::prompt_inspect::PromptInspect;

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
    /// Last assembled request, shared with [`AgentHandle::last_prompt`].
    pub(super) last_prompt: Arc<Mutex<Option<PromptInspect>>>,
    /// Guarded-mode work-order store shared with the tool context.
    /// `None` in unguarded runs, where no order digest is ever pinned.
    pub(super) work_orders: Option<Arc<crate::governance::WorkOrderStore>>,
    /// This run's `.z-engine/runs/<run-id>/` directory, where completion
    /// verification writes its manifest beside the evidence that produced
    /// it. `None` in unguarded runs, which never verify.
    pub(super) run_dir: Option<std::path::PathBuf>,
}

impl LoopState {
    /// The order this run is working under, if any.
    pub(super) fn active_work_order(&self) -> Option<Arc<crate::governance::ActiveWorkOrder>> {
        self.work_orders.as_ref()?.active()
    }

    /// Digest of the order this run is working under, pinned into every
    /// request while it is active. `None` in unguarded runs and before an
    /// order is accepted, which keeps those prompts byte-identical to
    /// what they were before governance existed.
    pub(super) fn work_order_digest(&self) -> Option<String> {
        Some(self.active_work_order()?.digest())
    }

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

/// Minimal state for tests of the pieces that only read a field or two,
/// so they need neither a provider nor a running loop.
#[cfg(test)]
impl LoopState {
    pub(super) fn for_test(run_dir: Option<std::path::PathBuf>) -> Self {
        Self {
            working: Vec::new(),
            approval_counter: 0,
            last_usage: z_engine_provider::Usage::default(),
            force_compact: false,
            repo_map_text: None,
            current_task: String::new(),
            reasoning_effort: None,
            last_prompt: Arc::new(Mutex::new(None)),
            work_orders: None,
            run_dir,
        }
    }
}
