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

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::context::cost::Pricing;

/// Fully resolved configuration after layering.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub model: String,
    pub base_url: String,
    /// Approximate context budget in tokens used by the status meter and,
    /// from v0.3 on, the compactor.
    pub max_context_tokens: u32,
    /// Explicit per-request output ceiling sent as max_tokens.
    pub max_output_tokens: u32,
    /// Auto-compaction trigger point as a percent of `max_context_tokens`
    /// (default 92, clamped to 80..=99).
    pub compact_at_percent: u8,
    /// Lifecycle hooks: event name → shell command. Honored events:
    /// `session_start`, `turn_completed`. stdout becomes a status note.
    pub hooks: BTreeMap<String, String>,
    pub permissions: PermissionsConfig,
    /// Post-edit reviewer pass (spec section 9 v0.9).
    pub review_enabled: bool,
    /// Post-edit reviewer pass (spec section 9 v0.9).
    /// MCP stdio servers (spec section 9 v0.9).
    pub mcp_servers: Vec<crate::mcp::McpServerConfig>,
    /// Per-model pricing overrides (exact model id → pricing). Takes
    /// precedence over the built-in table in [`Self::pricing_for`].
    pub cost_overrides: BTreeMap<String, Pricing>,
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
    pub hooks: Option<BTreeMap<String, String>>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub max_context_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub compact_at_percent: Option<u8>,
    pub review_enabled: Option<bool>,
    pub mcp_servers: Option<Vec<crate::mcp::McpServerConfig>>,
    pub permissions_allow: Option<Vec<String>>,
    pub cost_overrides: Option<BTreeMap<String, Pricing>>,
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
            max_output_tokens: 16_384,
            compact_at_percent: 92,
            hooks: BTreeMap::new(),
            permissions: PermissionsConfig::default(),
            review_enabled: true,
            mcp_servers: Vec::new(),
            cost_overrides: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Load configuration from the process environment + config files +
    /// CLI overrides. `project_root` enables the project-level
    /// `.harness/config.toml` layer (spec section 8).
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
        let project = project_root.map(project_config_path);
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
            let partial = parse_partial(text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
            apply(&mut cfg, &partial);
        }

        // project file: allow rules UNION, scalars override
        if let (Some(path), Some(text)) = (project_path, project_text) {
            let partial = parse_partial(text).map_err(|source| ConfigError::Parse {
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

    /// Resolved pricing for a model: exact-match override first, then the
    /// built-in substring table; `None` when truly unknown.
    pub fn pricing_for(&self, model: &str) -> Option<Pricing> {
        if let Some(p) = self.cost_overrides.get(model) {
            return Some(*p);
        }
        crate::context::cost::for_model(model)
    }
}

/// Path of the project-level config: `<project>/.harness/config.toml`.
pub fn project_config_path(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".harness").join("config.toml")
}

const PROJECT_CONFIG_HEADER: &str = "# harness project configuration\n# bash prefix rules under [permissions.allow] skip approval for this project.\n";

/// Persist a bash prefix rule into `<project>/.harness/config.toml`
/// (spec section 5, fourth modal answer). Values survive; comments in an
/// existing file are not preserved. Duplicate rules are ignored.
pub fn persist_bash_rule(project_root: &std::path::Path, rule: &str) -> std::io::Result<PathBuf> {
    let path = project_config_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    let mut allow = fmt
        .permissions
        .as_ref()
        .and_then(|p| p.allow.clone())
        .unwrap_or_default();
    if !allow.iter().any(|r| r == rule) {
        allow.push(rule.to_string());
    }
    let mut perms = fmt.permissions.take().unwrap_or_default();
    perms.allow = Some(allow);
    fmt.permissions = Some(perms);

    write_project_config(&path, &fmt)?;
    Ok(path)
}

/// List bash prefix rules persisted in `<project>/.harness/config.toml`.
pub fn list_bash_rules(project_root: &std::path::Path) -> std::io::Result<Vec<String>> {
    let path = project_config_path(project_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    Ok(fmt.permissions.and_then(|p| p.allow).unwrap_or_default())
}

/// Remove a bash prefix rule from `<project>/.harness/config.toml`.
/// Missing file or absent rule are treated as success (idempotent).
pub fn remove_bash_rule(project_root: &std::path::Path, rule: &str) -> std::io::Result<()> {
    let path = project_config_path(project_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    if let Some(perms) = fmt.permissions.as_mut() {
        if let Some(list) = perms.allow.as_mut() {
            list.retain(|r| r != rule);
        }
    }
    write_project_config(&path, &fmt)
}

/// Scalar settings editable from the Settings → General tab.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeneralOverrides {
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub max_context_tokens: Option<u32>,
    pub review_enabled: Option<bool>,
}

/// Persist general settings into `<project>/.harness/config.toml`,
/// preserving every other section. `None` fields are left untouched.
pub fn persist_general(
    project_root: &std::path::Path,
    over: &GeneralOverrides,
) -> std::io::Result<PathBuf> {
    if over.model.is_none()
        && over.base_url.is_none()
        && over.max_context_tokens.is_none()
        && over.review_enabled.is_none()
    {
        return Ok(project_config_path(project_root));
    }
    let path = project_config_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    if let Some(m) = &over.model {
        fmt.model = Some(m.clone());
    }
    if let Some(b) = &over.base_url {
        fmt.base_url = Some(b.clone());
    }
    if let Some(t) = over.max_context_tokens {
        fmt.max_context_tokens = Some(t);
    }
    if let Some(r) = over.review_enabled {
        fmt.review = Some(r);
    }
    write_project_config(&path, &fmt)?;
    Ok(path)
}

/// Persist a per-model pricing override into
/// `<project>/.harness/config.toml` under `[cost.overrides]`.
pub fn set_cost_override(
    project_root: &std::path::Path,
    model: &str,
    pricing: Pricing,
) -> std::io::Result<PathBuf> {
    let path = project_config_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    let mut section = fmt.cost.take().unwrap_or_default();
    section.overrides.insert(model.to_string(), pricing);
    fmt.cost = Some(section);
    write_project_config(&path, &fmt)?;
    Ok(path)
}

/// Remove a per-model pricing override (idempotent).
pub fn remove_cost_override(project_root: &std::path::Path, model: &str) -> std::io::Result<()> {
    let path = project_config_path(project_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    if let Some(mut cost) = fmt.cost.take() {
        cost.overrides.remove(model);
        fmt.cost = Some(cost);
    }
    write_project_config(&path, &fmt)
}

fn write_project_config(path: &std::path::Path, fmt: &FileFormat) -> std::io::Result<()> {
    let serialized = toml::to_string_pretty(fmt)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // Round-tripping drops comments/formatting; we re-add our standard
    // header so the file is always self-describing.
    let body = format!("{PROJECT_CONFIG_HEADER}{serialized}");
    std::fs::write(path, body)
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
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct FileFormat {
    hooks: Option<BTreeMap<String, String>>,
    model: Option<String>,
    base_url: Option<String>,
    max_context_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
    compact_at_percent: Option<u8>,
    review: Option<bool>,
    permissions: Option<FilePermissions>,
    mcp: Option<McpFileSection>,
    cost: Option<CostFileSection>,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct McpFileSection {
    #[serde(default)]
    servers: std::collections::BTreeMap<String, McpServerEntry>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct McpServerEntry {
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct FilePermissions {
    allow: Option<Vec<String>>,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct CostFileSection {
    #[serde(default)]
    overrides: BTreeMap<String, Pricing>,
}

fn parse_partial(text: &str) -> Result<PartialConfig, toml::de::Error> {
    let f: FileFormat = toml::from_str(text)?;
    Ok(PartialConfig {
        hooks: f.hooks,
        model: f.model,
        base_url: f.base_url,
        max_context_tokens: f.max_context_tokens,
        max_output_tokens: f.max_output_tokens,
        compact_at_percent: f.compact_at_percent,
        review_enabled: f.review,
        permissions_allow: f.permissions.and_then(|p| p.allow),
        cost_overrides: f.cost.map(|c| c.overrides),
        mcp_servers: f.mcp.map(|m| {
            m.servers
                .into_iter()
                .map(|(name, e)| crate::mcp::McpServerConfig {
                    name,
                    command: e.command,
                    args: e.args,
                })
                .collect()
        }),
    })
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

    #[test]
    fn persisted_rules_roundtrip_and_dedupe() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        persist_bash_rule(root, "cargo test*").unwrap();
        persist_bash_rule(root, "cargo test*").unwrap(); // dedupe
        persist_bash_rule(root, "git status").unwrap();

        let text = std::fs::read_to_string(project_config_path(root)).unwrap();
        assert!(text.starts_with("# harness"));
        // Layering must now load exactly these rules.
        let cfg = Config::load(&CliOverrides::default(), Some(root)).unwrap();
        assert_eq!(cfg.permissions.allow, vec!["cargo test*", "git status"]);
        // And the engine allows accordingly.
        assert!(
            cfg.permissions
                .allow
                .iter()
                .any(|r| rule_like(r, "cargo test --lib"))
        );
    }

    fn rule_like(rule: &str, cmd: &str) -> bool {
        match rule.strip_suffix('*') {
            Some(p) => cmd.starts_with(p.trim_end()),
            None => cmd == rule,
        }
    }

    #[test]
    fn malformed_project_config_blocks_persistence() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".harness")).unwrap();
        std::fs::write(root.join(".harness/config.toml"), "model = ").unwrap();
        let err = persist_bash_rule(root, "ls*");
        assert!(err.is_err());
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

    #[test]
    fn cost_overrides_merge_later_layers_win_and_pricing_prefers_them() {
        let global = r#"
[cost.overrides]
"my/model" = { usd_per_mtok_input = 1.0, usd_per_mtok_output = 2.0 }
"#;
        let project = r#"
[cost.overrides]
"my/model" = { usd_per_mtok_input = 9.0, usd_per_mtok_output = 9.5 }
"other/m" = { usd_per_mtok_input = 0.5, usd_per_mtok_output = 1.5 }
"#;
        let tmp = tempfile::tempdir().unwrap();
        let cfg = Config::layer_all(
            Some(std::path::Path::new("/tmp/g.toml")),
            Some(global),
            Some(&tmp.path().join(".harness/config.toml")),
            Some(project),
            &env(None, None),
            &CliOverrides::default(),
        )
        .unwrap();
        let p = cfg.pricing_for("my/model").unwrap();
        assert_eq!((p.usd_per_mtok_input, p.usd_per_mtok_output), (9.0, 9.5));
        // exact override beats the built-in substring table
        let built_in = crate::context::cost::for_model("claude-sonnet-4").unwrap();
        std::fs::create_dir_all(tmp.path().join(".harness")).unwrap();
        set_cost_override(
            tmp.path(),
            "anthropic/claude-sonnet-4",
            Pricing {
                usd_per_mtok_input: 42.0,
                usd_per_mtok_output: 43.0,
            },
        )
        .unwrap();
        let reloaded = Config::load(&CliOverrides::default(), Some(tmp.path())).unwrap();
        let over = reloaded.pricing_for("anthropic/claude-sonnet-4").unwrap();
        assert_eq!(over.usd_per_mtok_input, 42.0);
        assert_ne!(over, built_in);
        // unknown model without override stays unknown
        assert!(reloaded.pricing_for("nope/model").is_none());
    }

    #[test]
    fn cost_override_remove_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        remove_cost_override(tmp.path(), "x/y").unwrap(); // missing file ok
        set_cost_override(
            tmp.path(),
            "x/y",
            Pricing {
                usd_per_mtok_input: 1.0,
                usd_per_mtok_output: 2.0,
            },
        )
        .unwrap();
        remove_cost_override(tmp.path(), "x/y").unwrap();
        remove_cost_override(tmp.path(), "x/y").unwrap(); // absent rule ok
        let cfg = Config::load(&CliOverrides::default(), Some(tmp.path())).unwrap();
        assert!(cfg.cost_overrides.is_empty());
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
        std::fs::create_dir_all(tmp.path().join(".harness")).unwrap();
        std::fs::write(tmp.path().join(".harness/config.toml"), "model = ").unwrap();
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
