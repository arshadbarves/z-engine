//! A submitted goal's lifecycle: record intake, run a response, assess, commit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use z_engine_provider::{ChatMessage, Client};

use crate::context::{budget::BudgetMeter, notes::NotesStore};
use crate::session::{SessionEvent, SessionWriter};
use crate::tools::{ToolCtx, ToolRegistry};
use crate::verification::TaskStatus;

use super::LoopConfig;
use super::events::{Command, Event};
use super::hooks::run_hook;
use super::side_requests::generate_session_title;
use super::state::LoopState;
use super::task_completion;
use super::turn::{TurnOutcome, run_turn};

#[allow(clippy::too_many_arguments)]
pub(super) async fn submit(
    cfg: &LoopConfig,
    client: &Client,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    state: &mut LoopState,
    cmd_rx: &mut UnboundedReceiver<Command>,
    ev_tx: &UnboundedSender<Event>,
    abort_flag: &Arc<AtomicBool>,
    meter: &BudgetMeter,
    notes: &Arc<Mutex<NotesStore>>,
    recorder: &mut Option<SessionWriter>,
    titled: &mut bool,
    user_text: String,
    images: Vec<String>,
) {
    abort_flag.store(false, Ordering::Relaxed);
    state.current_task = user_text.clone();
    ctx.begin_checkpoint_turn();
    if let Some(writer) = recorder.as_mut() {
        if let Err(error) = writer.record(&SessionEvent::UserMsg {
            text: user_text.clone(),
            images: images.clone(),
        }) {
            let _ = ev_tx.send(Event::Error(format!(
                "Could not record task request: {error}"
            )));
            return;
        }
    }
    state
        .working
        .push(ChatMessage::user_with_images(&user_text, &images));
    let _ = ev_tx.send(Event::TurnStarted);
    let baseline = match task_completion::begin(ctx, &user_text, recorder, ev_tx).await {
        Ok(baseline) => baseline,
        Err(error) => {
            let _ = ev_tx.send(Event::Error(error.to_string()));
            return;
        }
    };
    if !*titled {
        *titled = true;
        spawn_title(client, cfg, &user_text, recorder, ev_tx);
    }
    let outcome = run_turn(
        cfg,
        client,
        registry,
        ctx,
        state,
        cmd_rx,
        ev_tx,
        abort_flag,
        meter,
        notes,
        recorder,
        baseline.as_ref(),
    )
    .await;
    let (turn_end, terminal) = match &outcome {
        TurnOutcome::Completed => ("completed", None),
        TurnOutcome::Aborted => ("aborted", Some(TaskStatus::Stopped)),
        TurnOutcome::Failed(_) => ("failed", Some(TaskStatus::Interrupted)),
    };
    let mut recording_error = None;
    if matches!(outcome, TurnOutcome::Completed)
        && !abort_flag.load(Ordering::Relaxed)
        && cfg.hooks.contains_key("turn_completed")
    {
        if let Err(error) = task_completion::block(
            ctx,
            "A lifecycle hook has unverified external effects.".into(),
        ) {
            recording_error = Some(error.to_string());
        }
        run_hook(&cfg.hooks, "turn_completed", &cfg.project_root, ev_tx).await;
    }
    if let Some(writer) = recorder.as_mut() {
        if let Err(error) = writer.record(&SessionEvent::TurnEnd {
            outcome: turn_end.into(),
        }) {
            recording_error = Some(format!("Could not record response outcome: {error}"));
            if let Err(state_error) = task_completion::block(ctx, error.to_string()) {
                tracing::error!(%state_error, "could not mark task persistence failure");
            }
        }
    }
    let final_status =
        match task_completion::finish(ctx, baseline.as_ref(), terminal, recorder, ev_tx).await {
            Ok(status) => Some(status),
            Err(error) => {
                recording_error = Some(error.to_string());
                None
            }
        };
    abort_flag.store(false, Ordering::Relaxed);
    if let Some(error) = recording_error {
        let _ = ev_tx.send(Event::Error(error));
        return;
    }
    if final_status == Some(TaskStatus::Stopped) {
        let _ = ev_tx.send(Event::TurnAborted);
        return;
    }
    match outcome {
        TurnOutcome::Completed => {
            let _ = ev_tx.send(Event::TurnCompleted {
                prompt_tokens: state.last_usage.prompt_tokens,
                completion_tokens: state.last_usage.completion_tokens,
            });
        }
        TurnOutcome::Aborted => {
            let _ = ev_tx.send(Event::TurnAborted);
        }
        TurnOutcome::Failed(message) => {
            let _ = ev_tx.send(Event::Error(message));
        }
    }
}

fn spawn_title(
    client: &Client,
    cfg: &LoopConfig,
    prompt: &str,
    recorder: &Option<SessionWriter>,
    events: &UnboundedSender<Event>,
) {
    let client = client.clone();
    let model = cfg.model.clone();
    let prompt = prompt.to_string();
    let events = events.clone();
    let path = recorder.as_ref().map(|writer| writer.path.clone());
    tokio::spawn(async move {
        let title = generate_session_title(&client, &model, &prompt)
            .await
            .unwrap_or_else(|| crate::session::fallback_title(&prompt));
        if let Some(path) = path {
            let result = SessionWriter::append_to(&path).and_then(|mut writer| {
                writer.record(&SessionEvent::Title {
                    text: title.clone(),
                })
            });
            if let Err(error) = result {
                let _ = events.send(Event::StatusNote(format!(
                    "Could not persist session title: {error}"
                )));
            }
        }
        let _ = events.send(Event::SessionTitle { text: title });
    });
}
