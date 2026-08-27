//! On-disk locations for Z Engine, with read fallbacks to the former
//! `harness` paths so existing installs keep working.

use std::path::{Path, PathBuf};

use super::types::EnvVars;

pub const APP_SLUG: &str = "z-engine";
pub const LEGACY_SLUG: &str = "harness";
pub const PROJECT_DIR: &str = ".z-engine";
pub const LEGACY_PROJECT_DIR: &str = ".harness";

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

/// Directory used for new project-local writes.
pub fn project_dir_write(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_DIR)
}

/// `<project>/.z-engine/config.toml` — always the write target.
pub fn project_config_path(project_root: &Path) -> PathBuf {
    project_dir_write(project_root).join("config.toml")
}

/// Prefer the new project config; fall back to `.harness/config.toml`.
pub fn project_config_read_path(project_root: &Path) -> PathBuf {
    let neu = project_config_path(project_root);
    let old = project_root.join(LEGACY_PROJECT_DIR).join("config.toml");
    first_existing(&[neu.clone(), old]).unwrap_or(neu)
}

/// `~/.config/z-engine` on Unix; `%APPDATA%\z-engine` on Windows, with
/// `~/.config` still read if it already exists.
pub fn global_config_dir() -> Option<PathBuf> {
    let mut cands = Vec::new();
    if let Some(home) = dirs::home_dir() {
        cands.push(home.join(".config").join(APP_SLUG));
        cands.push(home.join(".config").join(LEGACY_SLUG));
    }
    if let Some(cfg) = dirs::config_dir() {
        cands.push(cfg.join(APP_SLUG));
        cands.push(cfg.join(LEGACY_SLUG));
    }
    Some(first_existing(&cands).unwrap_or_else(default_global_config_dir))
}

fn default_global_config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::config_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(std::env::temp_dir)
            .join(APP_SLUG)
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".config")
            .join(APP_SLUG)
    }
}

/// Path of the global config file. `ZENGINE_CONFIG` / `HARNESS_CONFIG` win.
pub fn global_config_path(env: &EnvVars) -> Option<PathBuf> {
    if let Some(p) = &env.harness_config {
        return Some(PathBuf::from(p));
    }
    Some(global_config_dir()?.join("config.toml"))
}

/// Always the new data directory — writes (sessions, logs, caches) go here.
pub fn app_data_write_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_SLUG)
}

/// Platform data dir for reads: `…/z-engine`, else `…/harness`.
pub fn app_data_dir() -> PathBuf {
    let neu = app_data_write_dir();
    let old = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(LEGACY_SLUG);
    first_existing(&[neu.clone(), old]).unwrap_or(neu)
}

/// New session directory (create/append target).
pub fn sessions_dir() -> PathBuf {
    app_data_write_dir().join("sessions")
}

/// New sessions dir first, then the legacy harness dir if it still exists.
pub fn session_search_dirs() -> Vec<PathBuf> {
    let neu = sessions_dir();
    let old = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(LEGACY_SLUG)
        .join("sessions");
    let mut dirs = vec![neu.clone()];
    if old != neu && old.is_dir() {
        dirs.push(old);
    }
    dirs
}

/// User slash-command folders: project new, project old, global new, global old.
pub fn slash_command_dirs(project_root: &Path) -> Vec<(String, PathBuf)> {
    let mut dirs = vec![
        (
            "project".to_string(),
            project_root.join(PROJECT_DIR).join("commands"),
        ),
        (
            "project".to_string(),
            project_root.join(LEGACY_PROJECT_DIR).join("commands"),
        ),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push((
            "global".to_string(),
            home.join(".config").join(APP_SLUG).join("commands"),
        ));
        dirs.push((
            "global".to_string(),
            home.join(".config").join(LEGACY_SLUG).join("commands"),
        ));
    }
    if let Some(cfg) = dirs::config_dir() {
        for slug in [APP_SLUG, LEGACY_SLUG] {
            let p = cfg.join(slug).join("commands");
            if !dirs.iter().any(|(_, existing)| existing == &p) {
                dirs.push(("global".to_string(), p));
            }
        }
    }
    dirs
}

/// `~/.config/z-engine/models.json`, falling back to the harness path.
pub fn models_override_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    let neu = home.join(".config").join(APP_SLUG).join("models.json");
    let old = home.join(".config").join(LEGACY_SLUG).join("models.json");
    first_existing(&[neu.clone(), old]).unwrap_or(neu)
}

/// `ZENGINE_API_KEY`, then `HARNESS_API_KEY`, then `api-key` files.
pub fn resolve_api_key() -> Option<String> {
    for name in ["ZENGINE_API_KEY", "HARNESS_API_KEY"] {
        if let Ok(k) = std::env::var(name) {
            let k = k.trim().to_string();
            if !k.is_empty() {
                return Some(k);
            }
        }
    }
    let home = dirs::home_dir()?;
    for slug in [APP_SLUG, LEGACY_SLUG] {
        let path = home.join(".config").join(slug).join("api-key");
        if let Ok(s) = std::fs::read_to_string(path) {
            let t = s.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
        if let Some(cfg) = dirs::config_dir() {
            let path = cfg.join(slug).join("api-key");
            if let Ok(s) = std::fs::read_to_string(path) {
                let t = s.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_read_falls_back_to_legacy() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(LEGACY_PROJECT_DIR)).unwrap();
        std::fs::write(
            root.join(LEGACY_PROJECT_DIR).join("config.toml"),
            "model = \"x\"\n",
        )
        .unwrap();
        let p = project_config_read_path(root);
        assert!(p.ends_with("config.toml"));
        assert!(p.to_string_lossy().contains(LEGACY_PROJECT_DIR));
        assert_eq!(
            project_config_path(root),
            root.join(PROJECT_DIR).join("config.toml")
        );
    }

    #[test]
    fn sessions_dir_writes_to_new_slug() {
        let p = sessions_dir();
        assert!(p.ends_with("sessions"));
        assert!(p.to_string_lossy().contains(APP_SLUG));
    }
}
