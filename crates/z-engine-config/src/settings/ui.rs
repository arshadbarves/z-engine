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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct UiSettings {
    pub task_report_view: TaskReportView,
    /// Output style name (from `output-styles/`); `None` is the default style.
    pub output_style: Option<String>,
}
