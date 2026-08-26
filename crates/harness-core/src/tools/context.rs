use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::{checkpoint, file_state};
use crate::perms::PolicyEngine;

/// Shared execution context threaded through every tool call.
#[derive(Clone)]
pub struct ToolCtx {
    /// Project root; relative paths resolve against it.
    pub project_root: PathBuf,
    /// Persistent working directory across `bash` calls within a session.
    pub shell_cwd: Arc<Mutex<PathBuf>>,
    /// Cooperative abort flag — checked by tools between/inside long waits.
    pub abort: Arc<AtomicBool>,
    /// Permission engine handle (session rules mutate through this).
    pub perms: Arc<Mutex<PolicyEngine>>,
    /// Directory for full-output temp files.
    pub tmp_dir: PathBuf,
    /// Read-before-edit enforcement + staleness detection.
    pub file_state: Arc<Mutex<file_state::FileStateTracker>>,
    /// L1 context notes shared with the agent loop.
    pub notes: Arc<Mutex<crate::context::notes::NotesStore>>,
    /// Set whenever a file is read/written so the repo map regenerates.
    pub repo_map_dirty: Arc<std::sync::atomic::AtomicBool>,
    /// Optional spawner for isolated sub-agent research loops (`task` tool).
    pub task_runner: Option<SubAgentRunner>,
    /// Language server when project supports one.
    pub lsp: Option<Arc<crate::lsp::LspClient>>,
    /// Rendered results of this round's editing tools, drained by the
    /// reviewer pass (spec section 9 v0.9).
    pub edit_journal: Arc<Mutex<Vec<String>>>,
    /// Per-turn pre-edit file snapshots backing rewind (`RevertLastTurn`).
    pub checkpoints: Arc<checkpoint::CheckpointStore>,
    /// Live tool output streaming (bash stdout tails etc).
    pub output_tx: Arc<tokio::sync::mpsc::UnboundedSender<ToolOutputChunk>>,
}

/// A chunk of live tool output emitted while a tool is running.
#[derive(Debug, Clone)]
pub struct ToolOutputChunk {
    pub tool_name: String,
    pub text: String,
}

/// Future-yielding executor for isolated sub-loops. Built by the agent
/// module (which owns the provider client) and attached to the context so
/// the `task` tool can delegate without knowing about providers.
pub type SubAgentFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>;
pub type SubAgentRunner = Arc<dyn Fn(String, u32) -> SubAgentFuture + Send + Sync>;

impl ToolCtx {
    pub fn new(project_root: PathBuf, perms: Arc<Mutex<PolicyEngine>>, tmp_dir: PathBuf) -> Self {
        let shell_cwd = Arc::new(Mutex::new(project_root.clone()));
        Self {
            project_root,
            shell_cwd,
            abort: Arc::new(AtomicBool::new(false)),
            perms,
            tmp_dir,
            file_state: Arc::new(Mutex::new(file_state::FileStateTracker::default())),
            notes: Arc::new(Mutex::new(crate::context::notes::NotesStore::default())),
            repo_map_dirty: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            task_runner: None,
            lsp: None,
            edit_journal: Arc::new(Mutex::new(Vec::new())),
            checkpoints: Arc::new(checkpoint::CheckpointStore::default()),
            output_tx: Arc::new(tokio::sync::mpsc::unbounded_channel().0),
        }
    }

    /// Open a checkpoint for the turn about to run.
    pub fn begin_checkpoint_turn(&self) {
        self.checkpoints.begin_turn();
    }

    /// Stash `resolved`'s current content so a later rewind can restore it.
    /// Called by mutating tools right before their first write to a path
    /// within a turn; repeated calls for the same path are no-ops.
    pub fn checkpoint_before_mutation(&self, resolved: &Path) {
        self.checkpoints.snapshot_file(resolved);
    }

    /// Drain recorded edit results (for the reviewer pass).
    pub fn take_edit_journal(&self) -> Vec<String> {
        self.edit_journal
            .lock()
            .map(|mut j| std::mem::take(&mut *j))
            .unwrap_or_default()
    }

    /// Attach a sub-agent runner (builder style).
    pub fn with_task_runner(mut self, runner: SubAgentRunner) -> Self {
        self.task_runner = Some(runner);
        self
    }

    /// Canonicalized best-effort containment check: does `p` (relative to
    /// project root or absolute) resolve inside the project? Used to force
    /// gating and disable persistence for outside-root targets.
    pub fn is_outside_root(&self, p: &Path) -> bool {
        let resolved = self.resolve(p);
        let canonical = if resolved.exists() {
            std::fs::canonicalize(&resolved).ok()
        } else {
            resolved
                .parent()
                .and_then(|parent| std::fs::canonicalize(parent).ok())
                .map(|parent| parent.join(resolved.file_name().unwrap_or_default()))
        };
        let Some(canonical) = canonical else {
            return true; // cannot anchor => treat as outside
        };
        let root =
            std::fs::canonicalize(&self.project_root).unwrap_or_else(|_| self.project_root.clone());
        !canonical.starts_with(&root)
    }

    /// Record a successful read so later edits of this path are permitted;
    /// also flags the repo map for regeneration.
    pub fn note_read(&self, path: &Path) {
        use std::sync::atomic::Ordering;
        if let Ok(mut fs) = self.file_state.lock() {
            let _ = fs.record_read(path);
        }
        self.repo_map_dirty.store(true, Ordering::Relaxed);
    }

    /// Files currently tracked (read/edited) this session.
    pub fn tracked_paths(&self) -> BTreeSet<PathBuf> {
        self.file_state
            .lock()
            .map(|fs| fs.tracked_paths())
            .unwrap_or_default()
    }

    /// Read-before-edit gate: Ok when never-existed reads are required or
    /// satisfied; Err carries the model-facing refusal text.
    pub fn require_read_for_mutation(
        &self,
        _tool: &'static str,
        resolved: &Path,
        exists: bool,
    ) -> Result<(), super::ToolError> {
        let guard = self
            .file_state
            .lock()
            .map_err(|_| super::ToolError::Failed("file state lock poisoned".into()))?;
        if !exists {
            return Ok(()); // creating new files needs no prior read
        }
        if !guard.was_read(resolved) {
            return Err(super::ToolError::Failed(format!(
                "refusing to modify {} without reading it first — call read_file on this path, then retry",
                resolved.display()
            )));
        }
        if guard.is_stale(resolved) {
            return Err(super::ToolError::Failed(format!(
                "{} changed on disk since your last read — re-read it before editing",
                resolved.display()
            )));
        }
        drop(guard);
        // Refresh the snapshot to the post-edit state after callers write;
        // they call note_read themselves.
        Ok(())
    }

    pub fn aborted(&self) -> bool {
        self.abort.load(Ordering::Relaxed)
    }

    /// Resolve a user-supplied path: absolute paths are used as-is,
    /// relative ones anchor at the project root.
    pub fn resolve(&self, p: &Path) -> PathBuf {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.project_root.join(p)
        }
    }
}

impl std::fmt::Debug for ToolCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCtx")
            .field("project_root", &self.project_root)
            .field("shell_cwd", &self.shell_cwd)
            .field("aborted", &self.aborted())
            .finish_non_exhaustive()
    }
}
