//! Client-side handles: command sender, event receiver, and task spawning.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use z_engine_provider::{ChatMessage, Client};

use super::LoopConfig;
use super::events::{Command, Event};
use super::prompt_inspect::PromptInspect;
use super::subagent::run_isolated;
use super::task::agent_task;
use crate::perms::PolicyEngine;
use crate::session::SessionWriter;
use crate::tools::ToolRegistry;

/// Cloneable sender-side handle for driving the agent.
#[derive(Debug, Clone)]
pub struct AgentHandle {
    cmd_tx: UnboundedSender<Command>,
    last_prompt: Arc<Mutex<Option<PromptInspect>>>,
}

type PromptSlot = Arc<Mutex<Option<PromptInspect>>>;

impl AgentHandle {
    pub fn submit(&self, text: impl Into<String>) {
        self.submit_with_images(text, Vec::new());
    }

    /// Submit a task with attached images (data URLs) for vision models.
    pub fn submit_with_images(&self, text: impl Into<String>, images: Vec<String>) {
        let _ = self.cmd_tx.send(Command::SubmitMessage {
            text: text.into(),
            images,
        });
    }

    pub fn approve(&self, id: u64, decision: crate::agent::events::ApprovalDecision) {
        let _ = self.cmd_tx.send(Command::Approve { id, decision });
    }

    pub fn deny(&self, id: u64) {
        let _ = self.cmd_tx.send(Command::Deny { id });
    }

    pub fn abort(&self) {
        let _ = self.cmd_tx.send(Command::Abort);
    }

    /// Set the permission mode (Shift+Tab).
    pub fn set_mode(&self, mode: crate::agent::events::PermissionMode) {
        let _ = self.cmd_tx.send(Command::SetMode(mode));
    }

    /// Hot-switch the provider model (`/model <id>`).
    pub fn set_model(&self, model: impl Into<String>) {
        let _ = self.cmd_tx.send(Command::SetModel(model.into()));
    }

    /// Pick the reasoning effort for reasoning-capable models; `None`
    /// stops sending the parameter entirely.
    pub fn set_reasoning_effort(&self, effort: Option<String>) {
        let _ = self.cmd_tx.send(Command::SetReasoningEffort(effort));
    }

    /// Force context compaction now (`/compact`).
    pub fn compact(&self) {
        let _ = self.cmd_tx.send(Command::Compact);
    }

    /// Dump the current L1 notes (`/notes`).
    pub fn request_notes(&self) {
        let _ = self.cmd_tx.send(Command::RequestNotes);
    }

    /// Rewind: restore files touched by the last checkpointed turn.
    pub fn revert_last_turn(&self) {
        let _ = self.cmd_tx.send(Command::RevertLastTurn);
    }

    /// Per-message revert: restore all file changes from run-turn `keep`
    /// (the user message being reverted) and every later turn.
    pub fn revert_to_turn(&self, keep: u64) {
        let _ = self.cmd_tx.send(Command::RevertToTurn(keep));
    }

    /// `!<cmd>` local shell passthrough (never reaches the model).
    pub fn shell(&self, cmd: impl Into<String>) {
        let _ = self.cmd_tx.send(Command::Shell(cmd.into()));
    }

    /// Ask the loop task to finish gracefully.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(Command::Shutdown);
    }

    /// Last assembled chat-completion request (preview until a turn runs).
    pub fn last_prompt(&self) -> Option<PromptInspect> {
        self.last_prompt.lock().ok().and_then(|g| g.clone())
    }
}

/// Receiver end of the core→UI event feed.
#[derive(Debug)]
pub struct EventRx {
    rx: UnboundedReceiver<Event>,
}

impl EventRx {
    pub async fn recv(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    pub fn try_recv(&mut self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

/// Spawn the agent task. Returns a command handle and the event feed.
pub fn spawn(cfg: LoopConfig) -> (AgentHandle, EventRx) {
    spawn_with_recorder(cfg, None, None)
}

/// Preloaded conversation state for `--resume`.
#[derive(Debug, Default)]
pub struct ResumeState {
    pub working: Vec<ChatMessage>,
    /// Raw note payloads: either `update_context_notes` argument objects or
    /// plain compaction-summary lines.
    pub note_payloads: Vec<String>,
}

/// Spawn with an optional session recorder (persistence) and optional
/// replayed state (resume).
pub fn spawn_with_recorder(
    cfg: LoopConfig,
    resume: Option<ResumeState>,
    recorder: Option<SessionWriter>,
) -> (AgentHandle, EventRx) {
    let abort_flag = Arc::new(AtomicBool::new(false));

    // Sub-agent runner shares the provider client and the parent's abort
    // flag so Esc tears down the whole subtree.
    let sub_client = match Client::new(&cfg.base_url, cfg.api_key.clone()) {
        Ok(c) => c,
        Err(e) => {
            let (_cmd_tx, _cmd_rx) = mpsc::unbounded_channel::<Command>();
            let (ev_tx, ev_rx) = mpsc::unbounded_channel::<Event>();
            tokio::spawn(async move {
                let _ = ev_tx.send(Event::Error(format!("provider init failed: {e}")));
            });
            return (
                AgentHandle {
                    cmd_tx: _cmd_tx,
                    last_prompt: empty_prompt_slot(),
                },
                EventRx { rx: ev_rx },
            );
        }
    };
    let model = cfg.model.clone();
    let project_root = cfg.project_root.clone();
    let tmp_dir = cfg.tmp_dir.clone();
    let max_output = cfg.max_output_tokens;
    let sub_abort = Arc::clone(&abort_flag);
    let runner: crate::tools::SubAgentRunner = Arc::new(move |prompt: String, max_rounds: u32| {
        let client = sub_client.clone();
        let model = model.clone();
        let root = project_root.clone();
        let tmp = tmp_dir.clone();
        let abort = Arc::clone(&sub_abort);
        Box::pin(async move {
            run_isolated(
                client, model, root, tmp, abort, &prompt, max_rounds, max_output,
            )
            .await
        })
    });

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (ev_tx, ev_rx) = mpsc::unbounded_channel::<Event>();
    let last_prompt = Arc::new(Mutex::new(Some(PromptInspect::preview(
        &cfg,
        ToolRegistry::builtins().defs(),
    ))));

    match Client::new(&cfg.base_url, cfg.api_key.clone()) {
        Ok(client) => {
            let perms = Arc::new(Mutex::new(PolicyEngine::new(
                cfg.initial_allow_rules.clone(),
            )));
            let registry = ToolRegistry::builtins();
            tokio::spawn(agent_task(
                cfg,
                client,
                perms,
                registry,
                cmd_rx,
                ev_tx,
                resume,
                recorder,
                runner,
                abort_flag,
                Arc::clone(&last_prompt),
            ));
        }
        Err(e) => {
            // Surface asynchronously so callers can still attach to events.
            let ev_tx2 = ev_tx.clone();
            tokio::spawn(async move {
                let _ = ev_tx2.send(Event::Error(format!("provider init failed: {e}")));
            });
        }
    }
    (
        AgentHandle {
            cmd_tx,
            last_prompt,
        },
        EventRx { rx: ev_rx },
    )
}

fn empty_prompt_slot() -> PromptSlot {
    Arc::new(Mutex::new(None))
}
