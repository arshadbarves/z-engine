//! Layered settings, v1 migration, credentials, workspace trust, and
//! discovery of user-authored extensions and instruction files.
//!
//! Settings layer as defaults < user `config.toml` < project
//! `.z-engine/config.toml` < `.z-engine/config.local.toml` < environment.
//! Loading never fails: a broken layer is skipped and reported. Public
//! types derive `ts_rs::TS`; `cargo test -p z-engine-config` regenerates the
//! TypeScript mirror under `ui/src/lib/protocol/config/`.

pub mod credentials;
mod document;
mod error;
pub mod extensions;
mod files;
pub mod instructions;
mod loader;
mod merge;
mod migrate;
mod paths;
pub mod settings;
mod trust;
pub mod writer;

pub use credentials::{
    Credentials, KeyStatus, credential_key, resolve_api_key, resolve_api_key_with,
    resolve_search_key, resolve_search_key_with, search_key_bucket,
};
pub use document::SCHEMA_VERSION;
pub use error::ConfigError;
pub use extensions::{
    AgentDef, CommandDef, ExtensionError, ExtensionScope, ExtensionSource, Extensions,
    OutputStyleDef, RuleDef, SkillDef, discover_extensions, load_skill_body, parse_agent,
    parse_command,
};
pub use instructions::{
    InstructionFile, InstructionScope, discover_instructions, nested_instructions,
};
pub use loader::{EnvOverrides, LayerInfo, LayerScope, LoadedSettings, load, load_with_env};
pub use migrate::{MigrationOutcome, migrate_file};
pub use paths::{
    CONFIG_DIR_ENV, DATA_DIR_ENV, PROJECT_DIR, Paths, project_config_file, project_dir,
    project_local_file,
};
pub use settings::{
    AgentSettings, CheckConfig, CompatSettings, ContextSettings, HOOK_EVENTS, HookConfig,
    LspServerConfig, LspSettings, McpServerConfig, McpSettings, ModelSettings, PermissionSettings,
    PricingOverride, ProviderKind, ProviderSettings, RuleKind, SearchBackend, Settings,
    ShellSettings, TaskReportView, UiSettings, VerificationSettings, WebSettings, is_hook_event,
};
pub use trust::TrustStore;
pub use writer::{
    add_permission_rule, add_to_array, remove_from_array, remove_mcp_server,
    remove_permission_rule, remove_value, set_hooks, set_mcp_server, set_value,
};
