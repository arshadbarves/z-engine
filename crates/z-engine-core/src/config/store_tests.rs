use super::*;
use crate::config::types::{Config, EnvVars};

#[test]
fn persisted_rules_roundtrip_and_dedupe() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    persist_bash_rule(root, "cargo test*").unwrap();
    persist_bash_rule(root, "cargo test*").unwrap(); // dedupe
    persist_bash_rule(root, "git status").unwrap();

    let text = std::fs::read_to_string(project_config_path(root)).unwrap();
    assert!(text.starts_with("# z-engine"));
    let cfg = Config::load(Some(root)).unwrap();
    assert_eq!(cfg.permissions.allow, vec!["cargo test*", "git status"]);
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
    )
    .unwrap();
    let p = cfg.pricing_for("my/model").unwrap();
    assert_eq!((p.usd_per_mtok_input, p.usd_per_mtok_output), (9.0, 9.5));
    // Exact override beats the built-in substring table.
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
    let reloaded = Config::load(Some(tmp.path())).unwrap();
    let over = reloaded.pricing_for("anthropic/claude-sonnet-4").unwrap();
    assert_eq!(over.usd_per_mtok_input, 42.0);
    assert_ne!(over, built_in);
    assert!(reloaded.pricing_for("nope/model").is_none());
}

#[test]
fn cost_override_remove_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    remove_cost_override(tmp.path(), "x/y").unwrap();
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
    remove_cost_override(tmp.path(), "x/y").unwrap();
    let cfg = Config::load(Some(tmp.path())).unwrap();
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
    let cfg = Config::load(Some(tmp.path())).unwrap();
    assert_eq!(cfg.mcp_servers.len(), 1);
    assert_eq!(cfg.mcp_servers[0].name, "fs");
    assert_eq!(cfg.mcp_servers[0].command, "uvx");
    assert_eq!(cfg.mcp_servers[0].args, vec!["mcp-server-git"]);
    remove_mcp_server(tmp.path(), "fs").unwrap();
    remove_mcp_server(tmp.path(), "missing").unwrap();
    let cfg = Config::load(Some(tmp.path())).unwrap();
    assert!(cfg.mcp_servers.is_empty());
    assert!(persist_mcp_server(tmp.path(), "  ", "npx", vec![]).is_err());
}
