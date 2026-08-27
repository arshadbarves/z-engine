use super::paths::{global_config_path, project_config_read_path};
use super::types::{CliOverrides, Config, ConfigError, EnvVars, PartialConfig};

impl Config {
    /// Load configuration from the process environment + config files +
    /// CLI overrides. `project_root` enables the project-level
    /// `.z-engine/config.toml` layer (spec section 8).
    pub fn load(
        cli: &CliOverrides,
        project_root: Option<&std::path::Path>,
    ) -> Result<Self, ConfigError> {
        let env = EnvVars::from_process_env();
        let global_path = global_config_path(&env);
        let global_text = match &global_path {
            Some(p) => match std::fs::read_to_string(p) {
                Ok(text) => Some(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => {
                    return Err(ConfigError::Read {
                        path: p.clone(),
                        source: e,
                    });
                }
            },
            None => None,
        };
        let project = project_root.map(project_config_read_path);
        let project_text = match &project {
            Some(p) => match std::fs::read_to_string(p) {
                Ok(t) => Some(t),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => {
                    return Err(ConfigError::Read {
                        path: p.clone(),
                        source: e,
                    });
                }
            },
            None => None,
        };
        Self::layer_all(
            global_path.as_deref(),
            global_text.as_deref(),
            project.as_deref(),
            project_text.as_deref(),
            &env,
            cli,
        )
    }

    /// Pure layering step — fully unit-testable without touching the FS.
    /// Kept for compatibility; delegates to [`Self::layer_all`].
    pub fn layer(
        global_path: Option<&std::path::Path>,
        global_text: Option<&str>,
        env: &EnvVars,
        cli: &CliOverrides,
    ) -> Result<Self, ConfigError> {
        Self::layer_all(global_path, global_text, None, None, env, cli)
    }

    /// Full ladder: defaults < global < project < env < CLI.
    /// Allow rules UNION across files; scalars override.
    pub fn layer_all(
        global_path: Option<&std::path::Path>,
        global_text: Option<&str>,
        project_path: Option<&std::path::Path>,
        project_text: Option<&str>,
        env: &EnvVars,
        cli: &CliOverrides,
    ) -> Result<Self, ConfigError> {
        // defaults
        let mut cfg = Config::default();

        // global file
        if let (Some(path), Some(text)) = (global_path, global_text) {
            let partial =
                super::types::parse_partial(text).map_err(|source| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;
            apply(&mut cfg, &partial);
        }

        // project file: allow rules UNION, scalars override
        if let (Some(path), Some(text)) = (project_path, project_text) {
            let partial =
                super::types::parse_partial(text).map_err(|source| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;
            let mut merged = std::mem::take(&mut cfg.permissions.allow);
            if let Some(rules) = &partial.permissions_allow {
                for r in rules {
                    if !merged.contains(r) {
                        merged.push(r.clone());
                    }
                }
            }
            apply(
                &mut cfg,
                &PartialConfig {
                    permissions_allow: None,
                    ..partial
                },
            );
            cfg.permissions.allow = merged;
        }

        // environment
        apply(
            &mut cfg,
            &PartialConfig {
                model: env.harness_model.clone(),
                base_url: env.harness_base_url.clone(),
                ..PartialConfig::default()
            },
        );

        // CLI flags
        apply(
            &mut cfg,
            &PartialConfig {
                model: cli.model.clone(),
                base_url: cli.base_url.clone(),
                ..PartialConfig::default()
            },
        );

        // Normalize: strip trailing slash so `{base}/chat/completions` joining
        // behaves regardless of how the user wrote it.
        while cfg.base_url.ends_with('/') {
            cfg.base_url.pop();
        }
        if cfg.base_url.is_empty() {
            cfg.base_url = Config::default().base_url;
        }
        Ok(cfg)
    }
}

fn apply(cfg: &mut Config, partial: &PartialConfig) {
    if let Some(v) = &partial.hooks {
        cfg.hooks = v.clone();
    }
    if let Some(v) = &partial.model {
        cfg.model = v.clone();
    }
    if let Some(v) = &partial.base_url {
        cfg.base_url = v.clone();
    }
    if let Some(v) = partial.max_context_tokens {
        cfg.max_context_tokens = v;
    }
    if let Some(v) = partial.max_output_tokens {
        // Guard against absurd values that defeat the purpose.
        cfg.max_output_tokens = v.clamp(256, 200_000);
    }
    if let Some(v) = partial.compact_at_percent {
        cfg.compact_at_percent = v.clamp(80, 99);
    }
    if let Some(v) = partial.review_enabled {
        cfg.review_enabled = v;
    }
    if let Some(v) = &partial.mcp_servers {
        // union by name; later layers win on command/args
        for srv in v {
            if let Some(existing) = cfg.mcp_servers.iter_mut().find(|s| s.name == srv.name) {
                existing.command = srv.command.clone();
                existing.args = srv.args.clone();
            } else {
                cfg.mcp_servers.push(crate::mcp::McpServerConfig {
                    name: srv.name.clone(),
                    command: srv.command.clone(),
                    args: srv.args.clone(),
                });
            }
        }
    }
    if let Some(v) = &partial.permissions_allow {
        cfg.permissions.allow = v.clone();
    }
    if let Some(v) = &partial.cost_overrides {
        // per-model overrides; later layers win per model id
        for (model, pricing) in v {
            cfg.cost_overrides.insert(model.clone(), *pricing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(model: Option<&str>, base: Option<&str>) -> EnvVars {
        EnvVars {
            harness_model: model.map(str::to_string),
            harness_base_url: base.map(str::to_string),
            harness_config: None,
        }
    }

    #[test]
    fn defaults_when_no_layers() {
        let cfg = Config::layer(None, None, &env(None, None), &CliOverrides::default()).unwrap();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn global_file_overrides_defaults() {
        let text = r#"
model = "qwen/qwen3-coder"
max_context_tokens = 64000

[permissions]
allow = ["cargo test*", "git status"]
"#;
        let cfg = Config::layer(
            Some(std::path::Path::new("/tmp/harness-test-config.toml")),
            Some(text),
            &env(None, None),
            &CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(cfg.model, "qwen/qwen3-coder");
        assert_eq!(cfg.max_context_tokens, 64000);
        assert_eq!(cfg.permissions.allow, vec!["cargo test*", "git status"]);
        // untouched default survives
        assert_eq!(cfg.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn precedence_env_beats_file_cli_beats_env() {
        let text = r#"
model = "from-file"
base_url = "http://from-file/v1/"
"#;
        let cfg = Config::layer(
            None,
            Some(text),
            &env(Some("from-env"), Some("http://from-env/v1")),
            &CliOverrides {
                model: Some("from-cli".into()),
                base_url: None,
            },
        )
        .unwrap();
        assert_eq!(cfg.model, "from-cli"); // cli > env
        assert_eq!(cfg.base_url, "http://from-env/v1"); // env > file, slash stripped
    }

    #[test]
    fn project_file_unions_allow_rules_and_overrides_scalars() {
        let global = "model = \"g\"\n\n[permissions]\nallow = [\"cargo test*\"]\n";
        let project =
            "max_context_tokens = 5000\n\n[permissions]\nallow = [\"npm run*\", \"cargo test*\"]\n";
        let tmp = tempfile::tempdir().unwrap();
        let cfg = Config::layer_all(
            Some(std::path::Path::new("/tmp/global.toml")),
            Some(global),
            Some(&tmp.path().join(".harness/config.toml")),
            Some(project),
            &env(None, None),
            &CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(cfg.model, "g");
        assert_eq!(cfg.max_context_tokens, 5000);
        assert_eq!(cfg.permissions.allow, vec!["cargo test*", "npm run*"]);
    }

    #[test]
    fn malformed_project_file_is_an_error_too() {
        let err = Config::layer_all(
            None,
            None,
            Some(std::path::Path::new("/tmp/p.toml")),
            Some("model = "),
            &env(None, None),
            &CliOverrides::default(),
        );
        assert!(matches!(err, Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn malformed_global_file_is_an_error_not_silently_ignored() {
        let err = Config::layer(
            Some(std::path::Path::new("/tmp/x.toml")),
            Some("model = "),
            &env(None, None),
            &CliOverrides::default(),
        );
        assert!(matches!(err, Err(ConfigError::Parse { .. })));
    }
}
