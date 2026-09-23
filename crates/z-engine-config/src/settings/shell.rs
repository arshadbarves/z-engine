//! `[shell]`: the shell that runs commands and its environment.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct ShellSettings {
    /// Shell executable; `None` detects one.
    pub path: Option<String>,
    /// Variables passed through from the app's environment. Unions across layers.
    pub env_passthrough: Vec<String>,
    /// Variables set for every command.
    pub env: BTreeMap<String, String>,
}
