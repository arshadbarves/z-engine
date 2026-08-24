//! Layered configuration.
//!
//! Precedence (lowest → highest), per spec §8:
//!
//! ```text
//! defaults  <  ~/.config/harness/config.toml  <  environment vars  <  CLI flags
//! ```
//!
//! The project-level `.harness/config.toml` joins the ladder in v0.5; the
//! types below are already shaped for it.
//!
//! The API key is **never** part of config — it comes exclusively from the
//! `HARNESS_API_KEY` environment variable when a provider client is built.

use std::path::PathBuf;

/// Fully resolved configuration after layering.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub model: String,
    pub base_url: String,
    /// Approximate context budget in tokens used by the status meter and,
    /// from v0.3 on, the compactor.
    pub max_context_tokens: u32,
    pub permissions: PermissionsConfig,
}

/// Allowlist rules. v0.1 semantics: entries are `bash` command-prefix rules
/// (`cargo test*` matches any command starting with `cargo test`; entries
/// without a trailing `*` match exactly).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PermissionsConfig {
    pub allow: Vec<String>,
}

/// Sparse overlay produced by any single layer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartialConfig {
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub max_context_tokens: Option<u32>,
    pub permissions_allow: Option<Vec<String>>,
}

/// CLI-provided overrides (`--model`, `--base-url`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CliOverrides {
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// Environment variables honored by the loader (injectable for tests).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnvVars {
    pub harness_model: Option<String>,
    pub harness_base_url: Option<String>,
    /// Test hook: redirect the global config file location.
    pub harness_config: Option<String>,
}

impl EnvVars {
    fn from_process_env() -> Self {
        Self {
            harness_model: std::env::var("HARNESS_MODEL").ok(),
            harness_base_url: std::env::var("HARNESS_BASE_URL").ok(),
            harness_config: std::env::var("HARNESS_CONFIG").ok(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed reading config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed parsing config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: "openrouter/auto".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            max_context_tokens: 120_000,
            permissions: PermissionsConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from the process environment + global file +
    /// CLI overrides.
    pub fn load(cli: &CliOverrides) -> Result<Self, ConfigError> {
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
                    })
                }
            },
            None => None,
        };
        Self::layer(global_path.as_deref(), global_text.as_deref(), &env, cli)
    }

    /// Pure layering step — fully unit-testable without touching the FS.
    pub fn layer(
        global_path: Option<&std::path::Path>,
        global_text: Option<&str>,
        env: &EnvVars,
        cli: &CliOverrides,
    ) -> Result<Self, ConfigError> {
        // defaults
        let mut cfg = Config::default();

        // global file
        if let (Some(path), Some(text)) = (global_path, global_text) {
            let partial = parse_partial(text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
            apply(&mut cfg, &partial);
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

/// Path of the global config file, honoring `HARNESS_CONFIG`.
/// Spec §8 pins it to `~/.config/harness/config.toml` (deliberately *not*
/// the platform config dir, so behavior is identical across machines).
pub fn global_config_path(env: &EnvVars) -> Option<PathBuf> {
    if let Some(p) = &env.harness_config {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".config").join("harness").join("config.toml"))
}

/// TOML shape accepted inside a config file.
#[derive(Debug, Default, serde::Deserialize)]
struct FileFormat {
    model: Option<String>,
    base_url: Option<String>,
    max_context_tokens: Option<u32>,
    permissions: Option<FilePermissions>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct FilePermissions {
    allow: Option<Vec<String>>,
}

fn parse_partial(text: &str) -> Result<PartialConfig, toml::de::Error> {
    let f: FileFormat = toml::from_str(text)?;
    Ok(PartialConfig {
        model: f.model,
        base_url: f.base_url,
        max_context_tokens: f.max_context_tokens,
        permissions_allow: f.permissions.and_then(|p| p.allow),
    })
}

fn apply(cfg: &mut Config, partial: &PartialConfig) {
    if let Some(v) = &partial.model {
        cfg.model = v.clone();
    }
    if let Some(v) = &partial.base_url {
        cfg.base_url = v.clone();
    }
    if let Some(v) = partial.max_context_tokens {
        cfg.max_context_tokens = v;
    }
    if let Some(v) = &partial.permissions_allow {
        cfg.permissions.allow = v.clone();
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
    fn malformed_global_file_is_an_error_not_silently_ignored() {
        let err = Config::layer(
            Some(std::path::Path::new("/tmp/x.toml")),
            Some("model = "),
            &env(None, None),
            &CliOverrides::default(),
        );
        assert!(matches!(err, Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn trailing_slash_normalized_but_empty_falls_back_to_default() {
        let cfg = Config::layer(
            None,
            None,
            &env(None, Some("///")),
            &CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(cfg.base_url, "https://openrouter.ai/api/v1");
    }
}
