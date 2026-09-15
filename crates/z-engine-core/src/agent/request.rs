//! Assemble a provider request from stable guidance, conversation history and
//! the newest grounded task observation. Ephemeral packets never enter replay.

use std::sync::Mutex;
use std::sync::atomic::Ordering;

use tokio::sync::mpsc::UnboundedSender;
use z_engine_provider::{ChatMessage, ChatRequest};

use crate::context::{self, notes::NotesStore, task_packet};
use crate::tools::{ToolCtx, ToolRegistry};

use super::{LoopConfig, events::Event, prompt_inspect::PromptInspect, state::LoopState};

#[derive(Debug, thiserror::Error)]
pub(super) enum RequestError {
    #[error("context notes lock poisoned during request assembly")]
    NotesLock,
    #[error(transparent)]
    Packet(#[from] task_packet::TaskPacketError),
    #[error(transparent)]
    Encoding(#[from] z_engine_context::ContextError),
}

pub(super) fn assemble(
    cfg: &LoopConfig,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    state: &mut LoopState,
    notes: &Mutex<NotesStore>,
    events: &UnboundedSender<Event>,
) -> Result<ChatRequest, RequestError> {
    if ctx.repo_map_dirty.swap(false, Ordering::Relaxed) || state.repo_map_text.is_none() {
        state.repo_map_text = Some(context::repo_map::refresh_repo_map(ctx));
    }
    let mut messages = Vec::with_capacity(state.working.len() + 3);
    messages.push(super::system_prompt::l0_message(cfg));
    if let Some(map) = state.repo_map_text.as_ref().filter(|map| !map.is_empty()) {
        messages.push(ChatMessage::user(map.clone()));
    }
    messages.extend(state.working.iter().cloned());
    let packet = {
        let notes = notes.lock().map_err(|_| RequestError::NotesLock)?;
        // A byte allocation, not a tokenizer guarantee. Protected task data is
        // retained even when larger; the packet explicitly accounts for overflow.
        let target_bytes = (cfg.max_context_tokens as usize).clamp(4096, 64 * 1024);
        task_packet::build_task_packet(ctx, &notes, target_bytes)?
    };
    if packet.budget.over_budget_bytes > 0 {
        let _ = events.send(Event::StatusNote(format!(
            "Protected task context exceeds its byte target by {}; requirements were retained.",
            packet.budget.over_budget_bytes
        )));
    }
    messages.push(ChatMessage::user(packet.to_json()?));
    let mut request = ChatRequest::new(cfg.model.clone(), messages)
        .with_tools(registry.defs())
        .with_max_tokens(cfg.max_output_tokens);
    if let Some(effort) = state.reasoning_effort.clone() {
        request = request.with_reasoning_effort(effort);
    }
    if let Ok(mut slot) = state.last_prompt.lock() {
        *slot = Some(PromptInspect::from_request(&request, true));
    }
    Ok(request)
}
