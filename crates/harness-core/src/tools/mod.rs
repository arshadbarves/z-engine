//! Tool subsystem: the [`Tool`] trait (the only extension seam), the
//! execution context handed to every tool, the registry advertised to the
//! model, and shared output-truncation plumbing.
//!
//! Loop contract reminders (spec §4.2):
//! - tool errors are *data*: they become tool-result messages so the model
//!   can self-correct — they never crash the loop;
//! - oversized outputs are truncated head+tail for the transcript while the
//!   full text lands in a temp file whose path is embedded in the result.

pub mod bash;
pub mod checkpoint;
pub mod context_notes;
pub mod edit_file;
pub mod file_state;
pub mod glob;
pub mod grep;
pub mod lsp_tools;
pub mod read_file;
pub mod task;
pub mod write_file;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::perms::PolicyEngine;

/// Character budget for a tool result entering the transcript.
pub const MAX_TOOL_OUTPUT_CHARS: usize = 16_000;

/// Errors from tool execution. Display strings go to the model verbatim.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input for {tool}: {problem}")]
    InvalidInput { tool: &'static str, problem: String },
    #[error("{0}")]
    Failed(String),
}

/// Result payload of a successful or failed run.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Exact text that enters the transcript as the tool-result content.
    pub result: String,
    /// One-line human summary for TUI events.
    pub summary: String,
    pub ok: bool,
}

impl ToolOutput {
    pub fn success(result: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            summary: summary.into(),
            ok: true,
        }
    }

    pub fn failure(result: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            summary: summary.into(),
            ok: false,
        }
    }
}

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
    ) -> Result<(), ToolError> {
        let guard = self
            .file_state
            .lock()
            .map_err(|_| ToolError::Failed("file state lock poisoned".into()))?;
        if !exists {
            return Ok(()); // creating new files needs no prior read
        }
        if !guard.was_read(resolved) {
            return Err(ToolError::Failed(format!(
                "refusing to modify {} without reading it first — call read_file on this path, then retry",
                resolved.display()
            )));
        }
        if guard.is_stale(resolved) {
            return Err(ToolError::Failed(format!(
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

/// The single extension seam (spec §4.1). Built-ins and future MCP tools
/// implement exactly this; the agent loop never knows the difference.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema object describing the input.
    fn parameters_schema(&self) -> serde_json::Value;
    /// Whether parallel execution alongside other safe tools is OK.
    /// Mutating/stateful tools (bash) return false.
    fn concurrency_safe(&self) -> bool {
        true
    }
    /// Rich human/model-facing preview shown in the approval modal for gated
    /// calls (e.g. a unified diff). None falls back to raw input JSON.
    fn approval_preview(&self, _input: &serde_json::Value, _ctx: &ToolCtx) -> Option<String> {
        None
    }
    async fn run(&self, input: serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError>;
}

/// Registry of available tools; produces the provider-facing definitions.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if !self.tools.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn names(&self) -> &[String] {
        &self.order
    }

    /// Read-only subset for sub-agents (spec section 9 v0.7): isolated
    /// explore/research loops cannot mutate the workspace.
    pub fn readonly_subset() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(read_file::ReadFileTool));
        reg.register(Arc::new(glob::GlobTool));
        reg.register(Arc::new(grep::GrepTool));
        reg
    }

    /// Definitions advertised in chat-completion requests.
    pub fn defs(&self) -> Vec<crate::provider::ToolDef> {
        self.order
            .iter()
            .filter_map(|n| self.tools.get(n))
            .map(|t| {
                crate::provider::ToolDef::function(t.name(), t.description(), t.parameters_schema())
            })
            .collect()
    }

    /// Built-in toolset (grows each version).
    pub fn builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(read_file::ReadFileTool));
        reg.register(Arc::new(bash::BashTool));
        reg.register(Arc::new(write_file::WriteFileTool));
        reg.register(Arc::new(edit_file::EditFileTool));
        reg.register(Arc::new(glob::GlobTool));
        reg.register(Arc::new(grep::GrepTool));
        reg.register(Arc::new(context_notes::UpdateContextNotesTool));
        reg.register(Arc::new(task::TaskTool));

        // Read-only subset used by sub-agents (spec section 9 v0.7):
        // isolated loops default to exploration-only capabilities.

        reg
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.order)
            .finish()
    }
}

/// Crash-safe file replacement: write to a temp sibling, flush it to
/// disk, then atomically rename over the target. A crash mid-write can
/// never leave the target truncated or half-written (POSIX rename is
/// atomic; on Windows same-volume renames are best-effort but still far
/// safer than in-place truncation).
pub(crate) async fn atomic_write(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| std::io::Error::other("target has no parent directory"))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        ulid::Ulid::new()
    ));
    let write = async {
        let mut f = tokio::fs::File::create(&tmp).await?;
        tokio::io::AsyncWriteExt::write_all(&mut f, bytes).await?;
        f.sync_all().await?;
        std::io::Result::Ok(())
    };
    if let Err(e) = write.await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    match tokio::fs::rename(&tmp, target).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

/// Unified diff text between two versions of a file.
pub fn unified_diff(old: &str, new: &str, display_path: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(2)
        .header(&format!("a/{display_path}"), &format!("b/{display_path}"))
        .to_string()
}

/// Truncate `output` to fit the transcript budget, preserving head and
/// tail, and park the complete text in a temp file referenced inline.
pub fn truncate_with_tempfile(output: &str, ctx: &ToolCtx) -> String {
    if output.chars().count() <= MAX_TOOL_OUTPUT_CHARS {
        return output.to_string();
    }

    let path = next_tempfile_path(ctx);
    if let Err(e) = std::fs::write(&path, output) {
        tracing::warn!(%e, "failed writing full tool output tempfile");
        // Fall back to hard truncation without a pointer.
    }

    let total = output.chars().count();
    let budget = MAX_TOOL_OUTPUT_CHARS.saturating_sub(160); // room for marker
    let head = budget * 60 / 100;
    let tail = budget - head;

    let mut out = String::with_capacity(MAX_TOOL_OUTPUT_CHARS);
    out.extend(output.chars().take(head));
    let omitted = total - head - tail;
    out.push_str(&format!(
        "\n[...truncated {omitted} chars; full output: {}]\n",
        path.display()
    ));
    out.extend(output.chars().skip(total - tail));
    out
}

/// Write the full output to its own file even when under budget? No — only
/// truncation spills to disk. This helper just names spill files.
fn next_tempfile_path(ctx: &ToolCtx) -> PathBuf {
    let dir = ctx.tmp_dir.join("harness");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("out-{}.log", ulid::Ulid::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ToolCtx {
        let tmp = tempfile::tempdir().unwrap();
        ToolCtx::new(
            tmp.path().to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tmp.path().to_path_buf(),
        )
    }

    #[test]
    fn short_output_passes_through_unmodified() {
        let c = ctx();
        assert_eq!(truncate_with_tempfile("hello", &c), "hello");
    }

    #[test]
    fn long_output_truncated_head_tail_with_marker_and_spill_file() {
        let c = ctx();
        let big: String = "x".repeat(50_000);
        let out = truncate_with_tempfile(&big, &c);

        assert!(out.len() < MAX_TOOL_OUTPUT_CHARS + 200);
        assert!(out.starts_with("xxxx"));
        assert!(out.ends_with("xxxx"));
        let marker_at = out.find("[...truncated ").unwrap();
        let path_start = out[marker_at..].find("/").map(|i| marker_at + i).unwrap();
        let path_end = out[path_start..].find(']').unwrap() + path_start;
        let spill = PathBuf::from(&out[path_start..path_end]);
        let full = std::fs::read_to_string(&spill).unwrap();
        assert_eq!(full.len(), 50_000);
    }

    #[test]
    fn multibyte_content_counted_by_chars_not_bytes() {
        let c = ctx();
        let big = "é".repeat(20_000); // 40k bytes, 20k chars > budget
        let out = truncate_with_tempfile(&big, &c);
        assert!(out.contains("[...truncated"));
    }
}
