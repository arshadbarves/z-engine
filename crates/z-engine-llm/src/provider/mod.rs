//! Provider configuration, endpoint detection and client construction.

mod build;
mod config;
mod detect;

pub use build::build_client;
pub use config::{ANTHROPIC_BASE_URL, ProviderConfig, ProviderKind, ReasoningStyle};
pub use detect::{detect_kind, provider_label};
