use std::path::{Path, PathBuf};
use std::sync::Mutex;
use z_engine_core::agent::AgentHandle;
use z_engine_core::config::Config;

/// Shared application state managed by Tauri.
#[derive(Default)]
pub(crate) struct GuiState {
    pub(crate) handle: Mutex<Option<AgentHandle>>,
    pub(crate) ctx: Mutex<Option<AppCtx>>,
    /// Model id the running agent was started with / hot-switched to.
    pub(crate) model: Mutex<String>,
}

#[derive(Clone)]
pub(crate) struct AppCtx {
    pub(crate) project_root: PathBuf,
}

pub(crate) fn resolve_api_key() -> Option<String> {
    z_engine_core::config::resolve_api_key()
}

pub(crate) fn build_loop_config(
    cfg: &Config,
    project_root: &Path,
) -> z_engine_core::agent::LoopConfig {
    z_engine_core::agent::LoopConfig {
        model: cfg.model.clone(),
        base_url: cfg.base_url.clone(),
        api_key: resolve_api_key(),
        project_root: project_root.to_path_buf(),
        tmp_dir: std::env::temp_dir(),
        initial_allow_rules: cfg.permissions.allow.clone(),
        max_context_tokens: cfg.max_context_tokens,
        max_output_tokens: cfg.max_output_tokens,
        hooks: cfg.hooks.clone(),
        compact_at_percent: cfg.compact_at_percent,
        keep_recent_messages: 12,
        review_enabled: cfg.review_enabled,
        mcp_servers: cfg.mcp_servers.clone(),
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
    }
}

// ---- workspaces (Codex-desktop style project roots) ------------------------

fn workspaces_file_write() -> PathBuf {
    z_engine_core::config::app_data_write_dir().join("workspaces.json")
}

fn workspaces_file_read() -> PathBuf {
    let neu = workspaces_file_write();
    let old = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("workspaces.json");
    if neu.exists() {
        neu
    } else if old.exists() {
        old
    } else {
        neu
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct WorkspacesFile {
    roots: Vec<PathBuf>,
}

pub(crate) fn load_workspaces() -> Vec<PathBuf> {
    std::fs::read_to_string(workspaces_file_read())
        .ok()
        .and_then(|t| serde_json::from_str::<WorkspacesFile>(&t).ok())
        .map(|w| w.roots)
        .unwrap_or_default()
}

pub(crate) fn save_workspaces(roots: &[PathBuf]) -> Result<(), String> {
    let path = workspaces_file_write();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Deduplicate, preserving order.
    let mut seen = Vec::new();
    for r in roots {
        if !seen.contains(r) {
            seen.push(r.clone());
        }
    }
    let text =
        serde_json::to_string_pretty(&WorkspacesFile { roots: seen }).map_err(|e| e.to_string())?;
    // Temp-file + rename so a crash mid-write cannot truncate the
    // workspace registry.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
