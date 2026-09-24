//! The model catalog: the cached models.dev payload plus local overrides
//! (`models.json`) at startup, and an on-demand refresh that downloads,
//! caches and re-merges.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use z_engine_config::Paths;
use z_engine_host::{atomic_write, read_text};
use z_engine_llm::{LlmError, ModelCatalog, fetch_models_dev};

use crate::error::EngineError;
use crate::session::Shared;
use crate::sync::write;

const CACHE_FILE: &str = "models-dev.json";
const MAX_CATALOG_BYTES: usize = 64 * 1024 * 1024;

fn cache_path(paths: &Paths) -> PathBuf {
    paths.cache_dir.join(CACHE_FILE)
}

/// `None` when neither a cached catalog nor overrides exist. Unreadable
/// files are logged and skipped: the catalog only refines defaults.
pub(crate) fn load_cached(paths: &Paths) -> Option<ModelCatalog> {
    let cached = read_optional(&cache_path(paths)).and_then(|text| {
        ModelCatalog::from_models_dev_json(&text)
            .inspect_err(|error| tracing::warn!(%error, "cached model catalog ignored"))
            .ok()
    });
    let overrides = read_optional(&paths.models_file);
    if cached.is_none() && overrides.is_none() {
        return None;
    }
    let mut catalog = cached.unwrap_or_default();
    if let Some(overrides) = overrides {
        merge(&mut catalog, &overrides);
    }
    Some(catalog)
}

pub(crate) async fn refresh(shared: &Shared) -> Result<Arc<ModelCatalog>, EngineError> {
    let http = reqwest::Client::builder()
        .build()
        .map_err(|error| LlmError::Config(format!("http client: {error}")))?;
    let raw = fetch_models_dev(&http).await?;
    let mut catalog = ModelCatalog::from_models_dev_json(&raw)?;
    atomic_write(&cache_path(&shared.paths), raw.as_bytes()).await?;
    match read_text(&shared.paths.models_file, MAX_CATALOG_BYTES).await {
        Ok(file) => merge(&mut catalog, &file.content),
        Err(error) if error.is_not_found() => {}
        Err(error) => tracing::warn!(%error, "model overrides ignored"),
    }
    let catalog = Arc::new(catalog);
    *write(&shared.catalog) = Some(Arc::clone(&catalog));
    Ok(catalog)
}

fn merge(catalog: &mut ModelCatalog, overrides: &str) {
    if let Err(error) = catalog.merge_overrides(overrides) {
        tracing::warn!(%error, "model overrides ignored");
    }
}

fn read_optional(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "catalog file unreadable");
            None
        }
    }
}
