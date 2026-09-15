use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedSender;
use z_engine_provider::{ChatMessage, Client};

use crate::context::{compact, notes::NotesStore};
use crate::session::{SessionEvent, SessionWriter};

use super::LoopConfig;
use super::auxiliary::AuxiliaryError;
use super::events::Event;
use super::side_requests::summarize_segment;
use super::state::LoopState;

#[derive(Debug, thiserror::Error)]
pub(super) enum CompactionError {
    #[error(transparent)]
    Planning(#[from] compact::CompactionError),
    #[error(transparent)]
    Summary(#[from] AuxiliaryError),
    #[error("compaction summarizer returned no usable summary; original context retained")]
    EmptySummary,
    #[error("compaction requires a durable session recorder; original context retained")]
    MissingRecorder,
    #[error("context notes lock poisoned during compaction; original context retained")]
    NotesLock,
    #[error("could not persist compaction summary; original context retained: {0}")]
    Storage(#[from] std::io::Error),
}

pub(super) fn elide_marked_outputs(
    messages: &mut [ChatMessage],
    notes: &Mutex<NotesStore>,
    tmp_dir: &Path,
) -> Result<usize, CompactionError> {
    let mut notes = notes.lock().map_err(|_| CompactionError::NotesLock)?;
    let count = compact::elide_droppable(messages, notes.droppable_ids(), tmp_dir)?;
    drop(notes.take_droppable_ids());
    Ok(count)
}

pub(super) async fn compact_working_set(
    client: &Client,
    cfg: &LoopConfig,
    state: &mut LoopState,
    notes: &Arc<Mutex<NotesStore>>,
    ev_tx: &UnboundedSender<Event>,
    recorder: &mut Option<SessionWriter>,
    abort: &Arc<AtomicBool>,
) -> Result<(), CompactionError> {
    let before = state.pressure_tokens();
    let outcome = compact::compact(&state.working, cfg.keep_recent_messages, &cfg.tmp_dir)?;
    let summary = if outcome.summarize_input.is_empty() {
        None
    } else {
        if recorder.is_none() {
            return Err(CompactionError::MissingRecorder);
        }
        Some(summarize_segment(client, cfg, &outcome.summarize_input, abort).await?)
    };
    if abort.load(Ordering::Relaxed) {
        return Err(AuxiliaryError::Cancelled.into());
    }
    let elided = outcome.elided_tool_outputs;
    commit_compaction(state, notes, recorder, outcome, summary)?;
    let after = state.estimate_working();
    let _ = ev_tx.send(Event::StatusNote(format!(
        "context compacted: ~{} -> ~{} tokens ({} tool outputs elided)",
        before, after, elided
    )));
    Ok(())
}

fn commit_compaction(
    state: &mut LoopState,
    notes: &Mutex<NotesStore>,
    recorder: &mut Option<SessionWriter>,
    outcome: compact::CompactionOutcome,
    summary: Option<String>,
) -> Result<(), CompactionError> {
    if !outcome.summarize_input.is_empty() {
        let summary = summary
            .filter(|summary| !summary.trim().is_empty())
            .ok_or(CompactionError::EmptySummary)?;
        let mut notes = notes.lock().map_err(|_| CompactionError::NotesLock)?;
        let writer = recorder.as_mut().ok_or(CompactionError::MissingRecorder)?;
        writer.record_durable(&SessionEvent::Note {
            text: summary.clone(),
        })?;
        notes.add_summary(summary);
    }
    state.working = outcome.messages;
    Ok(())
}

#[cfg(test)]
#[path = "compaction_tests.rs"]
mod tests;
