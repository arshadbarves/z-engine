//! OpenRouter credentials in `auth.json` next to the global config.
//! The key is never written into `config.toml`.

use super::paths::{auth_path, ensure_global_config};
use super::types::EnvVars;

pub const OPENROUTER: &str = "openrouter";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuthEntry {
    #[serde(rename = "type")]
    kind: String,
    key: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct AuthFile {
    #[serde(flatten)]
    providers: std::collections::BTreeMap<String, AuthEntry>,
}

/// Redacted view of a stored key for the Settings UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyStatus {
    pub has_key: bool,
    /// Last four characters of the key, when present.
    pub hint: Option<String>,
}

fn load(env: &EnvVars) -> AuthFile {
    let Some(path) = auth_path(env) else {
        return AuthFile::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(env: &EnvVars, file: &AuthFile) -> std::io::Result<()> {
    let path = auth_path(env).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "cannot resolve auth.json")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, format!("{text}\n"))
}

pub fn provider_key(env: &EnvVars, provider: &str) -> Option<String> {
    let key = load(env).providers.get(provider)?.key.trim().to_string();
    if key.is_empty() { None } else { Some(key) }
}

pub fn openrouter_key(env: &EnvVars) -> Option<String> {
    provider_key(env, OPENROUTER)
}

pub fn set_provider_key(env: &EnvVars, provider: &str, key: Option<&str>) -> std::io::Result<()> {
    let _ = ensure_global_config(env);
    let mut file = load(env);
    match key.map(str::trim).filter(|s| !s.is_empty()) {
        Some(k) => {
            file.providers.insert(
                provider.to_string(),
                AuthEntry {
                    kind: "api".into(),
                    key: k.to_string(),
                },
            );
        }
        None => {
            file.providers.remove(provider);
        }
    }
    save(env, &file)
}

pub fn set_openrouter_key(env: &EnvVars, key: Option<&str>) -> std::io::Result<()> {
    set_provider_key(env, OPENROUTER, key)
}

fn hint_for(key: &str) -> String {
    let n = key.chars().count();
    key.chars().skip(n.saturating_sub(4)).collect()
}

pub fn key_status(env: &EnvVars, provider: &str) -> KeyStatus {
    match provider_key(env, provider) {
        Some(k) => KeyStatus {
            has_key: true,
            hint: Some(hint_for(&k)),
        },
        None => KeyStatus {
            has_key: false,
            hint: None,
        },
    }
}

pub fn openrouter_status(env: &EnvVars) -> KeyStatus {
    key_status(env, OPENROUTER)
}

/// Process-env wrappers used by the GUI and TUI shells.
pub fn current_openrouter_status() -> KeyStatus {
    openrouter_status(&EnvVars::from_process_env())
}

pub fn set_current_openrouter_key(key: Option<&str>) -> std::io::Result<()> {
    set_openrouter_key(&EnvVars::from_process_env(), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_at(dir: &std::path::Path) -> EnvVars {
        EnvVars {
            harness_model: None,
            harness_base_url: None,
            harness_config: Some(dir.join("config.toml").to_string_lossy().into_owned()),
        }
    }

    #[test]
    fn set_openrouter_key_roundtrips_and_redacts() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_at(tmp.path());
        assert!(!openrouter_status(&env).has_key);
        set_openrouter_key(&env, Some("  sk-or-secret-abcd  ")).unwrap();
        assert_eq!(openrouter_key(&env).as_deref(), Some("sk-or-secret-abcd"));
        let st = openrouter_status(&env);
        assert!(st.has_key);
        assert_eq!(st.hint.as_deref(), Some("abcd"));
        let raw = std::fs::read_to_string(tmp.path().join("auth.json")).unwrap();
        assert!(raw.contains("openrouter"));
        assert!(raw.contains("sk-or-secret-abcd"));
        set_openrouter_key(&env, Some("")).unwrap();
        assert!(openrouter_key(&env).is_none());
        assert!(!openrouter_status(&env).has_key);
    }
}
