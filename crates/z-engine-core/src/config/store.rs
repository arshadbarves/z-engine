use std::path::PathBuf;

use super::paths::{project_config_path, project_config_read_path};
use super::types::{ConfigError, FileFormat, MAX_TASK_CONTINUATIONS, TaskReportView};
use crate::context::cost::Pricing;

const PROJECT_CONFIG_HEADER: &str = "# z-engine project configuration\n# bash prefix rules under [permissions.allow] skip approval for this project.\n";

fn read_project_text(project_root: &std::path::Path) -> std::io::Result<String> {
    let path = project_config_read_path(project_root);
    match std::fs::read_to_string(&path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Persist a bash prefix rule into `<project>/.z-engine/config.toml`
/// (spec section 5, fourth modal answer). Values survive; comments in an
/// existing file are not preserved. Duplicate rules are ignored.
pub fn persist_bash_rule(project_root: &std::path::Path, rule: &str) -> std::io::Result<PathBuf> {
    let path = project_config_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = read_project_text(project_root)?;
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

/// List bash prefix rules persisted in `<project>/.z-engine/config.toml`.
pub fn list_bash_rules(project_root: &std::path::Path) -> std::io::Result<Vec<String>> {
    let path = project_config_read_path(project_root);
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

/// Remove a bash prefix rule from `<project>/.z-engine/config.toml`.
/// Missing file or absent rule are treated as success (idempotent).
pub fn remove_bash_rule(project_root: &std::path::Path, rule: &str) -> std::io::Result<()> {
    let path = project_config_path(project_root);
    let text = read_project_text(project_root)?;
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
    pub max_task_continuations: Option<u32>,
    pub task_report_view: Option<TaskReportView>,
}

fn persist_general_to_path(
    path: &std::path::Path,
    over: &GeneralOverrides,
) -> std::io::Result<PathBuf> {
    if let Some(value) = over.max_task_continuations {
        if value > MAX_TASK_CONTINUATIONS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                ConfigError::InvalidTaskContinuations(value),
            ));
        }
    }
    if over.model.is_none()
        && over.base_url.is_none()
        && over.max_context_tokens.is_none()
        && over.review_enabled.is_none()
        && over.max_task_continuations.is_none()
        && over.task_report_view.is_none()
    {
        return Ok(path.to_path_buf());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let mut fmt: FileFormat = if text.trim().is_empty() {
        FileFormat::default()
    } else {
        toml::from_str(&text).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("cannot parse {}: {e}", path.display()),
            )
        })?
    };
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
    if let Some(value) = over.max_task_continuations {
        fmt.max_task_continuations = Some(value);
    }
    if let Some(view) = over.task_report_view {
        fmt.task_report_view = Some(view);
    }
    write_project_config(path, &fmt)?;
    Ok(path.to_path_buf())
}

/// Persist general settings into `<project>/.z-engine/config.toml`,
/// preserving every other section. `None` fields are left untouched.
pub fn persist_general(
    project_root: &std::path::Path,
    over: &GeneralOverrides,
) -> std::io::Result<PathBuf> {
    persist_general_to_path(&project_config_path(project_root), over)
}

/// Persist general settings into global `~/.config/z-engine/config.toml`.
pub fn persist_global_general(over: &GeneralOverrides) -> std::io::Result<PathBuf> {
    let env = super::types::EnvVars::from_process_env();
    let path = super::paths::global_config_path(&env).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot resolve global config path",
        )
    })?;
    persist_general_to_path(&path, over)
}

/// Persist an MCP stdio server into `<project>/.z-engine/config.toml`
/// under `[mcp.servers.<name>]`. Later writes replace the same name.
pub fn persist_mcp_server(
    project_root: &std::path::Path,
    name: &str,
    command: &str,
    args: Vec<String>,
) -> std::io::Result<PathBuf> {
    let name = name.trim();
    let command = command.trim();
    if name.is_empty() || command.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "mcp server needs a name and command",
        ));
    }
    let path = project_config_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = read_project_text(project_root)?;
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    let mut section = fmt.mcp.take().unwrap_or_default();
    section.servers.insert(
        name.to_string(),
        super::types::McpServerEntry {
            command: command.to_string(),
            args,
        },
    );
    fmt.mcp = Some(section);
    write_project_config(&path, &fmt)?;
    Ok(path)
}

/// Remove an MCP server by name (idempotent).
pub fn remove_mcp_server(project_root: &std::path::Path, name: &str) -> std::io::Result<()> {
    let path = project_config_path(project_root);
    let text = read_project_text(project_root)?;
    if text.is_empty() {
        return Ok(());
    }
    let mut fmt: FileFormat = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cannot parse {}: {e}", path.display()),
        )
    })?;
    if let Some(mcp) = fmt.mcp.as_mut() {
        mcp.servers.remove(name);
    }
    write_project_config(&path, &fmt)
}

/// Persist a per-model pricing override into
/// `<project>/.z-engine/config.toml` under `[cost.overrides]`.
pub fn set_cost_override(
    project_root: &std::path::Path,
    model: &str,
    pricing: Pricing,
) -> std::io::Result<PathBuf> {
    let path = project_config_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = read_project_text(project_root)?;
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
    let text = read_project_text(project_root)?;
    if text.is_empty() {
        return Ok(());
    }
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
    use std::io::Write;

    let serialized = toml::to_string_pretty(fmt)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // Round-tripping drops comments/formatting; we re-add our standard
    // header so the file is always self-describing.
    let body = format!("{PROJECT_CONFIG_HEADER}{serialized}");
    let tmp = path.with_extension(format!("toml.tmp-{}", ulid::Ulid::new()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let result = (|| {
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        if let Err(error) = std::fs::remove_file(&tmp) {
            tracing::warn!(?tmp, %error, "Could not remove temporary config file");
        }
    }
    result
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
