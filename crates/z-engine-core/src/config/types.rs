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
    /// Test / `ZENGINE_CONFIG` hook: redirect the global config file location.
    pub harness_config: Option<String>,
}

impl EnvVars {
    pub(super) fn from_process_env() -> Self {
        Self {
            harness_model: first_env(&["ZENGINE_MODEL"]),
            harness_base_url: first_env(&["ZENGINE_BASE_URL"]),
            harness_config: first_env(&["ZENGINE_CONFIG"]),
        }
    }
}

fn first_env(names: &[&str]) -> Option<String> {
    for n in names {
        if let Ok(v) = std::env::var(n) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
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
            model: "openrouter/free".to_string(),
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
    /// Resolved pricing for a model: exact-match override first, then the
    /// built-in substring table; `None` when truly unknown.
    pub fn pricing_for(&self, model: &str) -> Option<Pricing> {
        if let Some(p) = self.cost_overrides.get(model) {
            return Some(*p);
        }
        crate::context::cost::for_model(model)
    }
}

/// TOML shape accepted inside a config file.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub(super) struct FileFormat {
    pub(super) hooks: Option<BTreeMap<String, String>>,
    pub(super) model: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) max_context_tokens: Option<u32>,
    pub(super) max_output_tokens: Option<u32>,
    pub(super) compact_at_percent: Option<u8>,
    pub(super) review: Option<bool>,
    pub(super) permissions: Option<FilePermissions>,
    pub(super) mcp: Option<McpFileSection>,
    pub(super) cost: Option<CostFileSection>,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub(super) struct McpFileSection {
    #[serde(default)]
    pub(super) servers: std::collections::BTreeMap<String, McpServerEntry>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct McpServerEntry {
    pub(super) command: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub(super) struct FilePermissions {
    pub(super) allow: Option<Vec<String>>,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub(super) struct CostFileSection {
    #[serde(default)]
    pub(super) overrides: BTreeMap<String, Pricing>,
}

pub(super) fn parse_partial(text: &str) -> Result<PartialConfig, toml::de::Error> {
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
