//! API keys (`auth.json`) and workspace trust. Keys provided through the
//! environment count as present, since the engine resolves them first.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use tauri::State;
use z_engine_config::{
    Credentials, KeyStatus, SearchBackend, Settings, TrustStore, credential_key, search_key_bucket,
};

use crate::ipc::{IpcResult, fail};
use crate::state::AppState;

/// Buckets always reported, with the environment variable that fills each.
const BUCKETS: &[(&str, &str)] = &[
    ("openrouter", "OPENROUTER_API_KEY"),
    ("opencode", "OPENCODE_API_KEY"),
    ("anthropic", "ANTHROPIC_API_KEY"),
    ("openai", "OPENAI_API_KEY"),
    ("brave", "BRAVE_API_KEY"),
    ("tavily", "TAVILY_API_KEY"),
    ("exa", "EXA_API_KEY"),
];
/// Applies to the configured provider, whatever its bucket.
const ANY_PROVIDER_VAR: &str = "ZENGINE_API_KEY";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrustStatus {
    trusted: bool,
    project_defines: Vec<String>,
}

#[tauri::command]
pub(crate) fn credential_status(
    state: State<'_, AppState>,
) -> IpcResult<BTreeMap<String, KeyStatus>> {
    let paths = state.engine.paths();
    let credentials = Credentials::load(&paths.auth_file).map_err(fail)?;
    let provider = credential_key(&state.engine.settings(None).settings.provider.base_url);
    let env = |name: &str| std::env::var(name).is_ok_and(|value| !value.trim().is_empty());
    Ok(statuses(
        &credentials,
        &stored_buckets(&paths.auth_file),
        &provider,
        env,
    ))
}

/// Stores the key for the bucket of `base_url`; null removes it.
#[tauri::command]
pub(crate) fn save_api_key(
    base_url: String,
    key: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    save_key(&state, &credential_key(&base_url), key)
}

#[tauri::command]
pub(crate) fn save_search_key(
    backend: SearchBackend,
    key: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    let bucket = search_key_bucket(backend).ok_or("this search backend needs no key")?;
    save_key(&state, bucket, key)
}

#[tauri::command]
pub(crate) fn trust_status(
    project_root: String,
    state: State<'_, AppState>,
) -> IpcResult<TrustStatus> {
    let root = AppState::root_arg(&project_root)?;
    let store = TrustStore::load(&state.engine.paths().trust_file).map_err(fail)?;
    let project = state.engine.settings(Some(&root)).settings;
    let user = state.engine.settings(None).settings;
    Ok(TrustStatus {
        trusted: store.is_trusted(&root),
        project_defines: project_defines(&project, &user),
    })
}

#[tauri::command]
pub(crate) fn set_trust(
    project_root: String,
    trusted: bool,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    let root = AppState::root_arg(&project_root)?;
    let file = &state.engine.paths().trust_file;
    let mut store = TrustStore::load(file).map_err(fail)?;
    let changed = if trusted {
        store.trust(&root)
    } else {
        store.revoke(&root)
    };
    if changed {
        store.save(file).map_err(fail)?;
    }
    state.engine.reload_settings(Some(&root));
    Ok(())
}

fn save_key(state: &AppState, bucket: &str, key: Option<String>) -> IpcResult<()> {
    let file = &state.engine.paths().auth_file;
    let mut credentials = Credentials::load(file).map_err(fail)?;
    credentials.set(bucket, key);
    credentials.save(file).map_err(fail)?;
    state.engine.reload_settings(None);
    Ok(())
}

/// Bucket names in `auth.json`, including ones this version does not know.
fn stored_buckets(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeMap<String, serde_json::Value>>(&text).ok())
        .map(|entries| entries.into_keys().collect())
        .unwrap_or_default()
}

fn statuses(
    credentials: &Credentials,
    stored: &[String],
    provider_bucket: &str,
    env: impl Fn(&str) -> bool,
) -> BTreeMap<String, KeyStatus> {
    let mut out: BTreeMap<String, KeyStatus> = stored
        .iter()
        .map(|bucket| (bucket.clone(), credentials.status(bucket)))
        .collect();
    let from_env = KeyStatus {
        has_key: true,
        hint: None,
    };
    for (bucket, var) in BUCKETS {
        let status = credentials.status(bucket);
        let status = if !status.has_key && env(var) {
            from_env.clone()
        } else {
            status
        };
        out.insert(bucket.to_string(), status);
    }
    if env(ANY_PROVIDER_VAR) {
        let status = out
            .entry(provider_bucket.to_string())
            .or_insert(from_env.clone());
        if !status.has_key {
            *status = from_env;
        }
    }
    out
}

/// What the project layers add beyond the user level and would run once
/// trusted.
fn project_defines(project: &Settings, user: &Settings) -> Vec<String> {
    let mut defines = Vec::new();
    if project.hooks != user.hooks {
        defines.push("hooks".to_string());
    }
    if project.mcp != user.mcp {
        defines.push("mcp servers".to_string());
    }
    if project.verification.checks != user.verification.checks {
        defines.push("checks".to_string());
    }
    defines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_keys_count_as_present() {
        let mut credentials = Credentials::default();
        credentials.set("openai", Some("sk-stored-1234".into()));
        credentials.set("custom:localhost:8080", Some("local-key".into()));
        let stored = vec!["openai".to_string(), "custom:localhost:8080".to_string()];
        let env = |name: &str| name == "ANTHROPIC_API_KEY";
        let out = statuses(&credentials, &stored, "openrouter", env);
        assert_eq!(out["openai"].hint.as_deref(), Some("1234"));
        assert!(out["custom:localhost:8080"].has_key);
        assert!(out["anthropic"].has_key && out["anthropic"].hint.is_none());
        assert!(!out["openrouter"].has_key && !out["brave"].has_key);

        let any = |name: &str| name == ANY_PROVIDER_VAR;
        let out = statuses(&credentials, &stored, "custom:proxy", any);
        assert!(out["custom:proxy"].has_key);
        assert!(!out["openrouter"].has_key);
    }

    #[test]
    fn trust_lists_what_the_project_adds() {
        let user = Settings::default();
        let mut project = Settings::default();
        assert!(project_defines(&project, &user).is_empty());
        project.hooks.insert("Stop".into(), Vec::new());
        project
            .verification
            .checks
            .push(z_engine_config::CheckConfig {
                id: "test".into(),
                label: "Tests".into(),
                command: "cargo test".into(),
                ..Default::default()
            });
        assert_eq!(project_defines(&project, &user), ["hooks", "checks"]);
    }
}
