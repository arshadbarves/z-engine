//! The repository map system section: built once (lazily, before the first
//! request that needs it) and kept byte-stable so the prompt-cache prefix
//! survives; only a compaction or a settings reload invalidates it. Files
//! the main agent touched rank first.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use z_engine_context::{SourceFile, render_template, repo_map, supported_extension};
use z_engine_host::{glob, read_text, relative_display};
use z_engine_prompts::system::REPO_MAP;

use super::spec::RunContext;
use crate::session::SessionCore;
use crate::sync::lock;

/// Files walked (most recently modified first) before filtering.
const MAX_WALK: usize = 20_000;
const MAX_FILES: usize = 400;
/// Larger sources are usually generated; they are left out.
const MAX_FILE_BYTES: usize = 200 * 1024;
const READ_CONCURRENCY: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct RepoMapCache {
    /// `None`: not built yet (or invalidated); `Some(None)`: no map.
    value: Mutex<Option<Option<Arc<str>>>>,
    building: tokio::sync::Mutex<()>,
}

impl RepoMapCache {
    /// The rendered section, when one was built.
    pub(crate) fn current(&self) -> Option<Arc<str>> {
        lock(&self.value).clone().flatten()
    }

    pub(crate) fn invalidate(&self) {
        *lock(&self.value) = None;
    }

    fn is_built(&self) -> bool {
        lock(&self.value).is_some()
    }
}

/// Builds the section once for agents working in the project tree.
pub(crate) async fn ensure_repo_map(ctx: &RunContext) {
    if ctx.spec.worktree.is_some() {
        return;
    }
    let cache = &ctx.core.repo_map;
    if cache.is_built() {
        return;
    }
    let _building = cache.building.lock().await;
    if cache.is_built() {
        return;
    }
    let built = build(&ctx.core).await;
    *lock(&cache.value) = Some(built);
}

async fn build(core: &SessionCore) -> Option<Arc<str>> {
    let settings = core.settings();
    let context = &settings.settings.context;
    if !context.repo_map {
        return None;
    }
    let root = core.root.clone();
    let paths = sources(&root).await;
    let files: Vec<SourceFile> = futures::stream::iter(paths)
        .map(|path| {
            let root = root.clone();
            async move { source_file(&root, &path).await }
        })
        .buffered(READ_CONCURRENCY)
        .filter_map(|file| async move { file })
        .collect()
        .await;
    if files.is_empty() {
        return None;
    }
    let focus: Vec<String> = core
        .main
        .files
        .tracked()
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let budget = usize::try_from(context.repo_map_chars).unwrap_or(usize::MAX);
    let map = tokio::task::spawn_blocking(move || repo_map(&files, &focus, budget))
        .await
        .map_err(|error| tracing::warn!(%error, "repo map build failed"))
        .ok()?;
    if map.trim().is_empty() {
        return None;
    }
    Some(render_template(REPO_MAP, &[("map", &map)]).into())
}

/// Supported source files (gitignore-aware walk), newest first.
async fn sources(root: &Path) -> Vec<PathBuf> {
    let walk_root = root.to_path_buf();
    let walked =
        tokio::task::spawn_blocking(move || glob(&walk_root, "**/*", None, MAX_WALK)).await;
    let found = match walked {
        Ok(Ok(found)) => found.paths,
        Ok(Err(error)) => {
            tracing::debug!(%error, "repo map walk failed");
            return Vec::new();
        }
        Err(error) => {
            tracing::warn!(%error, "repo map walk failed");
            return Vec::new();
        }
    };
    found
        .into_iter()
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(supported_extension)
        })
        .take(MAX_FILES)
        .collect()
}

async fn source_file(root: &Path, path: &Path) -> Option<SourceFile> {
    let file = read_text(path, MAX_FILE_BYTES).await.ok()?;
    if file.truncated || file.lossy {
        return None;
    }
    Some(SourceFile {
        path: relative_display(root, path).replace('\\', "/"),
        text: file.content,
    })
}
