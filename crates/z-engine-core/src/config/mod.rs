//! Layered configuration.
//!
//! Precedence (lowest → highest), per spec §8:
//!
//! ```text
//! defaults  <  ~/.config/z-engine/config.toml  <  environment vars  <  CLI flags
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

pub use auth::{
    KeyStatus, current_openrouter_status, openrouter_status, set_current_openrouter_key,
    set_openrouter_key,
};
pub use paths::{
    app_data_dir, app_data_write_dir, auth_path, ensure_global_config, ensure_user_config,
    global_config_path, models_override_path, project_config_path, project_config_read_path,
    resolve_api_key, resolve_api_key_from, session_search_dirs, sessions_dir, slash_command_dirs,
};
pub use store::{
    GeneralOverrides, list_bash_rules, persist_bash_rule, persist_general, persist_mcp_server,
    remove_bash_rule, remove_cost_override, remove_mcp_server, set_cost_override,
};
pub use types::{CliOverrides, Config, ConfigError, EnvVars, PartialConfig, PermissionsConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::cost::Pricing;

    #[test]
    fn malformed_project_config_blocks_persistence() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".z-engine")).unwrap();
        std::fs::write(root.join(".z-engine/config.toml"), "model = ").unwrap();
        let err = persist_bash_rule(root, "ls*");
        assert!(err.is_err());
    }

    #[test]
    fn trailing_slash_normalized_but_empty_falls_back_to_default() {
        let cfg = Config::layer(
            None,
            None,
            &EnvVars {
                harness_model: None,
                harness_base_url: Some("///".to_string()),
                harness_config: None,
                harness_shell: None,
            },
            &CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(cfg.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn persist_general_writes_scalars_and_preserves_other_sections() {
        let tmp = tempfile::tempdir().unwrap();
        persist_bash_rule(tmp.path(), "cargo test*").unwrap();
        set_cost_override(
            tmp.path(),
            "m/x",
            Pricing {
                usd_per_mtok_input: 1.0,
                usd_per_mtok_output: 2.0,
            },
        )
        .unwrap();

        persist_general(
            tmp.path(),
            &GeneralOverrides {
                model: Some("z/a".into()),
                base_url: None,
                max_context_tokens: Some(64_000),
                review_enabled: Some(false),
            },
        )
        .unwrap();

        let cfg = Config::load(&CliOverrides::default(), Some(tmp.path())).unwrap();
        assert_eq!(cfg.model, "z/a");
        assert_eq!(cfg.max_context_tokens, 64_000);
        assert!(!cfg.review_enabled);
        // untouched sections survived the rewrite
        assert_eq!(cfg.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(cfg.permissions.allow, vec!["cargo test*"]);
        assert!(cfg.cost_overrides.contains_key("m/x"));
        // empty overrides is a no-op
        persist_general(tmp.path(), &GeneralOverrides::default()).unwrap();
    }

    #[test]
    fn malformed_project_config_blocks_general_and_cost_writes() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".z-engine")).unwrap();
        std::fs::write(tmp.path().join(".z-engine/config.toml"), "model = ").unwrap();
        assert!(
            persist_general(
                tmp.path(),
                &GeneralOverrides {
                    model: Some("m".into()),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(
            set_cost_override(
                tmp.path(),
                "m",
                Pricing {
                    usd_per_mtok_input: 1.0,
                    usd_per_mtok_output: 1.0
                }
            )
            .is_err()
        );
    }
}
