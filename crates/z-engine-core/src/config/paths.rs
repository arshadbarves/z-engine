//! On-disk locations for Z Engine. Missing files are created; there is
//! no fallback to the former `harness` config paths.

use std::path::{Path, PathBuf};

use super::types::EnvVars;

pub const APP_SLUG: &str = "z-engine";
pub const PROJECT_DIR: &str = ".z-engine";

const DEFAULT_GLOBAL_TOML: &str = "# z-engine configuration\n\
# OpenRouter API key lives in auth.json next to this file (Settings → General).\n\
model = \"openrouter/free\"\n\
base_url = \"https://openrouter.ai/api/v1\"\n";

/// Directory used for new project-local writes.
pub fn project_dir_write(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_DIR)
}

/// `<project>/.z-engine/config.toml` — always the write target.
pub fn project_config_path(project_root: &Path) -> PathBuf {
    project_dir_write(project_root).join("config.toml")
}

/// Same as [`project_config_path`] — project config is never read from `.harness`.
pub fn project_config_read_path(project_root: &Path) -> PathBuf {
    project_config_path(project_root)
}

/// `~/.config/z-engine` on Unix; `%APPDATA%\z-engine` on Windows.
pub fn global_config_dir() -> Option<PathBuf> {
    Some(default_global_config_dir())
}

/// Create the global config directory, `config.toml`, and `auth.json` when
/// they are missing. Existing files are left untouched.
pub fn ensure_global_config(env: &EnvVars) -> std::io::Result<PathBuf> {
    let path = global_config_path(env).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot resolve global config path",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        std::fs::write(&path, DEFAULT_GLOBAL_TOML)?;
    }
    let auth = auth_path_beside(&path);
    if !auth.exists() {
        std::fs::write(&auth, "{}\n")?;
    }
    Ok(path)
}

/// Create `~/.config/z-engine/{config.toml,auth.json}` if they are missing.
pub fn ensure_user_config() -> std::io::Result<PathBuf> {
    ensure_global_config(&EnvVars::from_process_env())
}

pub(crate) fn auth_path_beside(config_toml: &Path) -> PathBuf {
    config_toml
        .parent()
        .map(|d| d.join("auth.json"))
        .unwrap_or_else(|| PathBuf::from("auth.json"))
}

/// Path of `auth.json` next to the global config file.
pub fn auth_path(env: &EnvVars) -> Option<PathBuf> {
    Some(auth_path_beside(&global_config_path(env)?))
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

/// Path of the global config file. `ZENGINE_CONFIG` wins when set.
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

/// Platform data dir for reads (`…/z-engine`).
pub fn app_data_dir() -> PathBuf {
    app_data_write_dir()
}

/// New session directory (create/append target).
pub fn sessions_dir() -> PathBuf {
    app_data_write_dir().join("sessions")
}

/// Session directory used for listing and resume.
pub fn session_search_dirs() -> Vec<PathBuf> {
    vec![sessions_dir()]
}

/// User slash-command folders: project `.z-engine/commands`, then global.
pub fn slash_command_dirs(project_root: &Path) -> Vec<(String, PathBuf)> {
    let mut dirs = vec![(
        "project".to_string(),
        project_root.join(PROJECT_DIR).join("commands"),
    )];
    if let Some(home) = dirs::home_dir() {
        dirs.push((
            "global".to_string(),
            home.join(".config").join(APP_SLUG).join("commands"),
        ));
    }
    if let Some(cfg) = dirs::config_dir() {
        let p = cfg.join(APP_SLUG).join("commands");
        if !dirs.iter().any(|(_, existing)| existing == &p) {
            dirs.push(("global".to_string(), p));
        }
    }
    dirs
}

/// `~/.config/z-engine/models.json`.
pub fn models_override_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".config")
        .join(APP_SLUG)
        .join("models.json")
}

/// `ZENGINE_API_KEY`, then `auth.json` (OpenRouter) next to the global config.
pub fn resolve_api_key() -> Option<String> {
    resolve_api_key_from(&EnvVars::from_process_env())
}

/// Injectable variant of [`resolve_api_key`] for tests.
pub fn resolve_api_key_from(env: &EnvVars) -> Option<String> {
    if let Ok(k) = std::env::var("ZENGINE_API_KEY") {
        let t = k.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    super::auth::openrouter_key(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_read_ignores_legacy_harness_and_uses_z_engine() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".harness")).unwrap();
        std::fs::write(root.join(".harness").join("config.toml"), "model = \"x\"\n").unwrap();
        let p = project_config_read_path(root);
        assert_eq!(p, root.join(PROJECT_DIR).join("config.toml"));
        assert!(!p.to_string_lossy().contains(".harness"));
    }

    #[test]
    fn ensure_global_config_creates_toml_and_auth_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_path = tmp.path().join("config.toml");
        let env = EnvVars {
            harness_model: None,
            harness_base_url: None,
            harness_config: Some(cfg_path.to_string_lossy().into_owned()),
            harness_shell: None,
        };
        let wrote = ensure_global_config(&env).unwrap();
        assert_eq!(wrote, cfg_path);
        assert!(cfg_path.is_file());
        let text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(text.contains("openrouter"));
        let auth = tmp.path().join("auth.json");
        assert!(auth.is_file());
        assert_eq!(std::fs::read_to_string(&auth).unwrap().trim(), "{}");
        // second call must not clobber
        std::fs::write(&cfg_path, "model = \"kept\"\n").unwrap();
        ensure_global_config(&env).unwrap();
        assert_eq!(
            std::fs::read_to_string(&cfg_path).unwrap(),
            "model = \"kept\"\n"
        );
    }

    #[test]
    fn sessions_dir_writes_to_new_slug() {
        let p = sessions_dir();
        assert!(p.ends_with("sessions"));
        assert!(p.to_string_lossy().contains(APP_SLUG));
    }
}
