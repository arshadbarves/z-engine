use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use z_engine_core::agent::AgentHandle;
use z_engine_core::config::Config;

/// Shared application state managed by Tauri.
#[derive(Default)]
pub(crate) struct GuiState {
    /// One agent loop per session ULID so chats can run in the background.
    pub(crate) loops: Mutex<HashMap<String, AgentHandle>>,
    pub(crate) active: Mutex<String>,
    pub(crate) ctx: Mutex<Option<AppCtx>>,
    /// Model id the running agent was started with / hot-switched to.
    pub(crate) model: Mutex<String>,
}

impl GuiState {
    pub(crate) fn handle_for(&self, session_id: Option<&str>) -> Result<AgentHandle, String> {
        let id = match session_id {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => self.active.lock().map_err(|_| "state poisoned")?.clone(),
        };
        self.loops
            .lock()
            .map_err(|_| "state poisoned")?
            .get(&id)
            .cloned()
            .ok_or_else(|| "agent not started".into())
    }

    pub(crate) fn set_active(&self, id: String) -> Result<(), String> {
        *self.active.lock().map_err(|_| "state poisoned")? = id;
        Ok(())
    }

    pub(crate) fn has_loop(&self, id: &str) -> Result<bool, String> {
        Ok(self
            .loops
            .lock()
            .map_err(|_| "state poisoned")?
            .contains_key(id))
    }

    pub(crate) fn insert_loop(&self, id: String, handle: AgentHandle) -> Result<(), String> {
        self.loops
            .lock()
            .map_err(|_| "state poisoned")?
            .insert(id.clone(), handle);
        self.set_active(id)
    }

    pub(crate) fn shutdown_one(&self, id: &str) -> Result<(), String> {
        if let Some(h) = self.loops.lock().map_err(|_| "state poisoned")?.remove(id) {
            h.shutdown();
        }
        let active = self.active.lock().map_err(|_| "state poisoned")?;
        if active.as_str() == id {
            drop(active);
            self.set_active(String::new())?;
        }
        Ok(())
    }

    pub(crate) fn shutdown_all(&self) {
        if let Ok(mut loops) = self.loops.lock() {
            for (_, h) in loops.drain() {
                h.shutdown();
            }
        }
    }
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
    guarded: bool,
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
        guarded,
    }
}

// ---- workspaces (Codex-desktop style project roots) ------------------------

fn workspaces_file_write() -> PathBuf {
    z_engine_core::config::app_data_write_dir().join("workspaces.json")
}

fn workspaces_file_read() -> PathBuf {
    workspaces_file_write()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use z_engine_core::agent::PermissionMode;

    #[test]
    fn guarded_flag_reaches_loop_config() {
        let cfg = Config::default();
        let root = PathBuf::from("/tmp/proj");
        assert!(build_loop_config(&cfg, &root, true).guarded);
        assert!(!build_loop_config(&cfg, &root, false).guarded);
    }

    #[test]
    fn gui_uses_resolved_guarded_config_without_changing_permissions() {
        for (text, expected) in [
            ("", true),
            ("guarded = false", false),
            ("guarded = true", true),
        ] {
            let cfg = Config::layer(
                Some(Path::new("config.toml")),
                Some(text),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();
            let loop_cfg = build_loop_config(&cfg, Path::new("."), cfg.guarded);
            assert_eq!(loop_cfg.guarded, expected, "{text}");
            assert_eq!(loop_cfg.initial_mode, PermissionMode::Normal);
            assert!(loop_cfg.auto_allow_tools.is_empty());
        }
    }
}
