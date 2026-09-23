//! The v2 settings schema, one file per TOML section. TOML keys and the
//! GUI's JSON both use snake_case field names.

mod agents;
mod compat;
mod context;
mod hooks;
mod lenient;
mod lsp;
mod mcp;
mod model;
mod normalize;
mod permissions;
mod provider;
mod root;
mod shell;
mod ui;
mod unknown_keys;
mod verification;
mod web;

pub use agents::AgentSettings;
pub use compat::CompatSettings;
pub use context::{ContextSettings, MAX_COMPACT_AT_PERCENT, MIN_COMPACT_AT_PERCENT};
pub use hooks::{HOOK_EVENTS, HookConfig, is_hook_event};
pub use lsp::{LspServerConfig, LspSettings};
pub use mcp::{McpServerConfig, McpSettings};
pub use model::{
    DEFAULT_MODEL, MAX_OUTPUT_TOKENS, MIN_OUTPUT_TOKENS, ModelSettings, PricingOverride,
};
pub use permissions::{PermissionSettings, RuleKind};
pub use provider::{DEFAULT_BASE_URL, ProviderKind, ProviderSettings};
pub use root::Settings;
pub use shell::ShellSettings;
pub use ui::{TaskReportView, UiSettings};
pub use verification::{CheckConfig, MAX_CONTINUATIONS, VerificationSettings};
pub use web::{SearchBackend, WebSettings};

pub(crate) use normalize::normalize;
pub(crate) use unknown_keys::unknown_keys;
