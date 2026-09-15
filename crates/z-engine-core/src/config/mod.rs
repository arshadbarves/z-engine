//! Layered configuration.
//!
//! Precedence (lowest → highest), per spec §8:
//!
//! ```text
//! defaults  <  global config  <  project config  <  environment vars
//! ```
//!
//! Missing global files are created on startup. The OpenRouter API key is
//! stored in `auth.json` next to the global config (set from Settings),
//! with `ZENGINE_API_KEY` as an override.

mod auth;
mod loader;
mod paths;
mod store;
mod types;

#[cfg(test)]
mod supervision_tests;
#[cfg(test)]
mod task_report_view_tests;
#[cfg(test)]
mod tests;

pub use auth::{
    KeyStatus, OPENCODE, OPENROUTER, current_key_status_for_base_url, current_openrouter_status,
    key_status_for_base_url, openrouter_status, provider_id_for_base_url,
    set_current_key_for_base_url, set_current_openrouter_key, set_opencode_key, set_openrouter_key,
};
pub use paths::{
    app_data_dir, app_data_write_dir, auth_path, ensure_global_config, ensure_user_config,
    global_config_path, models_override_path, project_config_path, project_config_read_path,
    resolve_api_key, resolve_api_key_for, resolve_api_key_for_base_url, resolve_api_key_from,
    session_search_dirs, sessions_dir, slash_command_dirs,
};
pub use store::{
    GeneralOverrides, list_bash_rules, persist_bash_rule, persist_general, persist_global_general,
    persist_mcp_server, remove_bash_rule, remove_cost_override, remove_mcp_server,
    set_cost_override,
};
pub use types::{
    Config, ConfigError, EnvVars, MAX_TASK_CONTINUATIONS, PartialConfig, PermissionsConfig,
    TaskReportView,
};
