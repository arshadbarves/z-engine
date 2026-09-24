//! `[ui]`: presentation preferences.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum TaskReportView {
    #[default]
    Quiet,
    Compact,
    Detailed,
}

/// How much the title-bar companion reacts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum CompanionLevel {
    /// Reacts to the agent and to what you do.
    #[default]
    Lively,
    /// Reacts only to the agent.
    Calm,
    /// No companion; the status line keeps a small dot.
    Off,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct UiSettings {
    pub task_report_view: TaskReportView,
    /// Output style name (from `output-styles/`); `None` is the default style.
    pub output_style: Option<String>,
    pub companion: CompanionLevel,
}
