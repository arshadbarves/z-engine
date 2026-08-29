use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::path_identity::{canonical_in_root, canonicalize_root, to_repo_relative};
use super::{checkpoint, file_state};
use crate::evidence::{BlobHandle, BlobStore, EvidenceLedger, EvidenceRecord};
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
    /// Optional revision-scoped evidence recorder (guarded mode, Task 3+).
    /// `None` leaves reads behaving exactly as before this feature existed.
    pub evidence: Option<Arc<EvidenceStore>>,
}

/// Bundles the ledger and blob store used to record and check freshness of
/// file-read evidence, plus an in-memory index of the latest record per
/// path so repeated freshness checks don't need to replay the whole
/// on-disk ledger. This wires storage handles from the `evidence` module
/// onto `ToolCtx`; it reuses that module's hashing/CAS logic rather than
/// duplicating it.
pub struct EvidenceStore {
    ledger: Arc<EvidenceLedger>,
    blobs: Arc<dyn BlobStore + Send + Sync>,
    latest: Mutex<HashMap<PathBuf, EvidenceRecord>>,
}

impl EvidenceStore {
    pub fn new(ledger: Arc<EvidenceLedger>, blobs: Arc<dyn BlobStore + Send + Sync>) -> Self {
        Self {
            ledger,
            blobs,
            latest: Mutex::new(HashMap::new()),
        }
    }
}

impl std::fmt::Debug for EvidenceStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceStore").finish_non_exhaustive()
    }
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
            evidence: None,
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

    /// Attach a revision-scoped evidence recorder (builder style). Once
    /// set, successful bounded reads become durable, edit-authorizing
    /// evidence (Task 3+); leaving it unset preserves prior behavior.
    pub fn with_evidence(mut self, store: Arc<EvidenceStore>) -> Self {
        self.evidence = Some(store);
        self
    }

    /// Record one successful bounded read as immutable evidence: the
    /// returned range's bytes are stored once in the content-addressed
    /// blob store, a durable [`EvidenceRecord`] is appended to the
    /// ledger, and the in-memory freshness index is updated so later
    /// [`ToolCtx::fresh_read_evidence`] calls avoid re-scanning the
    /// on-disk ledger.
    ///
    /// `resolved_path` is canonicalized (symlinks resolved, `.`/`..`
    /// normalized) before being used as the record's path and the
    /// freshness-index key, so equivalent spellings of the same in-root
    /// file (`./f.rs`, `sub/../f.rs`, a symlink into the repo, ...) always
    /// key to the same evidence identity — see [`ToolCtx::fresh_read_evidence`].
    /// Returns `Ok(None)` — recording nothing — both when no evidence
    /// recorder is attached (unguarded mode) *and* when `resolved_path`
    /// canonicalizes outside the project root: evidence's `path` field is
    /// documented as repository-relative, so an outside-root read must
    /// never be forced into a fabricated "relative" spelling or silently
    /// authorize edits outside the project.
    ///
    /// Storage failures (once past those two skip cases) are typed and
    /// fail closed rather than silently dropping evidence a later guarded
    /// gate might otherwise trust.
    ///
    /// Callers must only invoke this for genuinely successful, non-binary
    /// reads — binary or failed reads must never become edit-authorizing
    /// evidence. `full_file_bytes` and `range_bytes` must both derive from
    /// the *same* read of the file the caller already used to build the
    /// displayed output — never from a second, independent read — so a
    /// concurrent write can never make the evidence describe bytes the
    /// model never saw.
    pub fn record_read_evidence(
        &self,
        resolved_path: &Path,
        line_range: Option<(u32, u32)>,
        full_file_bytes: &[u8],
        range_bytes: &[u8],
    ) -> Result<Option<String>, super::ToolError> {
        let Some(store) = &self.evidence else {
            return Ok(None);
        };
        let Some(canonical) = canonical_in_root(resolved_path, &self.project_root) else {
            return Ok(None); // outside-root reads are never recorded as evidence
        };
        let rel_path = to_repo_relative(&canonical, &canonicalize_root(&self.project_root));
        let file_hash = BlobHandle::of(full_file_bytes).to_string();
        let blob = store
            .blobs
            .put(range_bytes)
            .map_err(|e| super::ToolError::Failed(format!("recording read evidence: {e}")))?;
        let record = EvidenceRecord::new(
            rel_path,
            line_range,
            file_hash,
            blob,
            "read_file",
            git_head_or_working_tree(&self.project_root),
        );
        store
            .ledger
            .append(&record)
            .map_err(|e| super::ToolError::Failed(format!("recording read evidence: {e}")))?;
        let id = record.id.clone();
        if let Ok(mut latest) = store.latest.lock() {
            latest.insert(canonical, record);
        }
        Ok(Some(id))
    }

    /// The most recent read evidence for `path` (resolved and
    /// canonicalized the same way as [`ToolCtx::record_read_evidence`], so
    /// `./f.rs`, `sub/../f.rs`, and a symlink to `f.rs` all look up the
    /// same record), only if the file's content on disk still matches the
    /// hash captured at read time.
    ///
    /// `None` means no evidence recorder is attached, `path` canonicalizes
    /// outside the project root, nothing was ever read, or the file has
    /// since changed on disk — any of which must block edit-authorizing
    /// use of stale, foreign, or absent evidence.
    pub fn fresh_read_evidence(&self, path: &Path) -> Option<EvidenceRecord> {
        let store = self.evidence.as_ref()?;
        let resolved = self.resolve(path);
        let canonical = canonical_in_root(&resolved, &self.project_root)?;
        let record = {
            let latest = store.latest.lock().ok()?;
            latest.get(&canonical)?.clone()
        };
        let current = std::fs::read(&canonical).ok()?;
        let current_hash = BlobHandle::of(&current).to_string();
        (current_hash == record.file_hash).then_some(record)
    }

    /// Canonicalized best-effort containment check: does `p` (relative to
    /// project root or absolute) resolve inside the project? Used to force
    /// gating and disable persistence for outside-root targets.
    pub fn is_outside_root(&self, p: &Path) -> bool {
        let resolved = self.resolve(p);
        canonical_in_root(&resolved, &self.project_root).is_none()
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

/// Best-effort git HEAD for `root`; falls back to `"working-tree"` when the
/// directory isn't a git repository or the command fails for any reason —
/// evidence capture must never depend on `git` being present or working.
fn git_head_or_working_tree(root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "working-tree".to_string())
}
