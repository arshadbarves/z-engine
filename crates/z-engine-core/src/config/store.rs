use std::path::PathBuf;

use super::paths::{project_config_path, project_config_read_path};
use super::types::FileFormat;
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
}

/// Persist general settings into `<project>/.z-engine/config.toml`,
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
    let text = read_project_text(project_root)?;
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
    let serialized = toml::to_string_pretty(fmt)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // Round-tripping drops comments/formatting; we re-add our standard
    // header so the file is always self-describing.
    let body = format!("{PROJECT_CONFIG_HEADER}{serialized}");
    std::fs::write(path, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{CliOverrides, Config, EnvVars};

    #[test]
    fn persisted_rules_roundtrip_and_dedupe() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        persist_bash_rule(root, "cargo test*").unwrap();
        persist_bash_rule(root, "cargo test*").unwrap(); // dedupe
        persist_bash_rule(root, "git status").unwrap();

        let text = std::fs::read_to_string(project_config_path(root)).unwrap();
        assert!(text.starts_with("# z-engine"));
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
            Some(&tmp.path().join(".z-engine/config.toml")),
            Some(project),
            &EnvVars::default(),
            &CliOverrides::default(),
        )
        .unwrap();
        let p = cfg.pricing_for("my/model").unwrap();
        assert_eq!((p.usd_per_mtok_input, p.usd_per_mtok_output), (9.0, 9.5));
        // exact override beats the built-in substring table
        let built_in = crate::context::cost::for_model("claude-sonnet-4").unwrap();
        std::fs::create_dir_all(tmp.path().join(".z-engine")).unwrap();
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
    fn mcp_server_roundtrip_replace_and_remove() {
        let tmp = tempfile::tempdir().unwrap();
        persist_mcp_server(
            tmp.path(),
            "fs",
            "npx",
            vec![
                "-y".into(),
                "@modelcontextprotocol/server-filesystem".into(),
            ],
        )
        .unwrap();
        persist_mcp_server(tmp.path(), "fs", "uvx", vec!["mcp-server-git".into()]).unwrap();
        let cfg = Config::load(&CliOverrides::default(), Some(tmp.path())).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1);
        assert_eq!(cfg.mcp_servers[0].name, "fs");
        assert_eq!(cfg.mcp_servers[0].command, "uvx");
        assert_eq!(cfg.mcp_servers[0].args, vec!["mcp-server-git"]);
        remove_mcp_server(tmp.path(), "fs").unwrap();
        remove_mcp_server(tmp.path(), "missing").unwrap();
        let cfg = Config::load(&CliOverrides::default(), Some(tmp.path())).unwrap();
        assert!(cfg.mcp_servers.is_empty());
        assert!(persist_mcp_server(tmp.path(), "  ", "npx", vec![]).is_err());
    }
}
