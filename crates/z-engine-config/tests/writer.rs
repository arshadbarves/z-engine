//! GUI edits of one layer file: creation, round trips, preserved keys,
//! refused invalid writes, and v1 files migrated before editing.

#[allow(dead_code)]
mod support;

use support::{fixture, read, write};
use toml::Value;
use z_engine_config::{
    ConfigError, HookConfig, McpServerConfig, RuleKind, add_permission_rule, add_to_array,
    project_local_file, remove_from_array, remove_mcp_server, remove_permission_rule, remove_value,
    set_hooks, set_mcp_server, set_value,
};

#[test]
fn a_missing_file_is_created_with_the_schema() {
    let f = fixture();
    let file = project_local_file(&f.project);
    set_value(&file, &["model", "main"], Value::String("m".into())).unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert_eq!(doc["schema"].as_integer(), Some(2));
    assert_eq!(f.load().settings.model.main, "m");
}

#[test]
fn edits_keep_unrelated_keys() {
    let f = fixture();
    let file = f.paths.user_config_file.clone();
    let original = "schema = 2\nexperimental = { flag = true }\n[model]\nmain = \"m\"\n[mcp.servers.fs]\ncommand = \"npx\"\n[[hooks.Stop]]\ncommand = \"done\"\n";
    write(&file, original);
    set_value(&file, &["context", "repo_map"], Value::Boolean(false)).unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert_eq!(doc["experimental"]["flag"].as_bool(), Some(true));
    assert_eq!(doc["model"]["main"].as_str(), Some("m"));
    assert_eq!(doc["mcp"]["servers"]["fs"]["command"].as_str(), Some("npx"));
    assert_eq!(doc["hooks"]["Stop"][0]["command"].as_str(), Some("done"));
    assert_eq!(doc["context"]["repo_map"].as_bool(), Some(false));

    remove_value(&file, &["context", "repo_map"]).unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert!(doc.get("context").is_none(), "empty tables are pruned");
}

#[test]
fn arrays_deduplicate_and_keep_emptied_lists() {
    let f = fixture();
    let file = f.paths.user_config_file.clone();
    add_to_array(&file, &["model", "fallbacks"], "a").unwrap();
    add_to_array(&file, &["model", "fallbacks"], "b").unwrap();
    add_to_array(&file, &["model", "fallbacks"], "a").unwrap();
    assert_eq!(f.load().settings.model.fallbacks, ["a", "b"]);
    remove_from_array(&file, &["model", "fallbacks"], "a").unwrap();
    remove_from_array(&file, &["model", "fallbacks"], "b").unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert_eq!(doc["model"]["fallbacks"].as_array().map(Vec::len), Some(0));
}

#[test]
fn permission_rules_round_trip() {
    let f = fixture();
    let file = project_local_file(&f.project);
    add_permission_rule(&file, RuleKind::Allow, " Bash(cargo test:*) ").unwrap();
    add_permission_rule(&file, RuleKind::Allow, "Bash(cargo test:*)").unwrap();
    add_permission_rule(&file, RuleKind::Deny, "Read(./.env)").unwrap();
    let permissions = f.load().settings.permissions;
    assert_eq!(permissions.allow, ["Bash(cargo test:*)"]);
    assert_eq!(permissions.deny, ["Read(./.env)"]);
    remove_permission_rule(&file, RuleKind::Allow, "Bash(cargo test:*)").unwrap();
    assert!(f.load().settings.permissions.allow.is_empty());
    assert!(add_permission_rule(&file, RuleKind::Ask, "  ").is_err());
}

#[test]
fn mcp_servers_round_trip_and_invalid_servers_are_refused() {
    let f = fixture();
    let file = f.paths.user_config_file.clone();
    let stdio = McpServerConfig {
        command: Some("npx".into()),
        args: vec!["-y".into(), "fs".into()],
        ..McpServerConfig::default()
    };
    set_mcp_server(&file, "fs", &stdio).unwrap();
    assert_eq!(f.load().settings.mcp.servers["fs"], stdio);
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    let written = doc["mcp"]["servers"]["fs"].as_table().unwrap();
    let mut keys: Vec<_> = written.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["args", "command", "enabled", "timeout_secs"],
        "empty lists and maps are omitted"
    );

    let http = McpServerConfig {
        url: Some("http://localhost:3000/mcp".into()),
        ..McpServerConfig::default()
    };
    set_mcp_server(&file, "fs", &http).unwrap();
    assert_eq!(f.load().settings.mcp.servers["fs"], http);

    let before = read(&file);
    let both = McpServerConfig {
        command: Some("npx".into()),
        ..http
    };
    assert!(matches!(
        set_mcp_server(&file, "fs", &both),
        Err(ConfigError::Invalid { .. })
    ));
    assert_eq!(read(&file), before);

    remove_mcp_server(&file, "fs").unwrap();
    remove_mcp_server(&file, "missing").unwrap();
    assert!(f.load().settings.mcp.servers.is_empty());
}

#[test]
fn hooks_round_trip() {
    let f = fixture();
    let file = project_local_file(&f.project);
    let hook = HookConfig {
        matcher: Some("Edit|Write".into()),
        command: "./fmt.sh".into(),
        timeout_secs: 30,
    };
    set_hooks(&file, "PostToolUse", std::slice::from_ref(&hook)).unwrap();
    assert_eq!(f.load().settings.hooks["PostToolUse"], [hook]);
    set_hooks(&file, "PostToolUse", &[]).unwrap();
    assert!(f.load().settings.hooks.is_empty());
    let unknown = set_hooks(&file, "AfterLunch", &[HookConfig::default()]);
    assert!(matches!(unknown, Err(ConfigError::KeyPath { .. })));
}

#[test]
fn invalid_edits_are_refused_without_touching_the_file() {
    let f = fixture();
    let file = f.paths.user_config_file.clone();
    write(&file, "schema = 2\n[model]\nmain = \"m\"\n");
    let before = read(&file);
    let bad_value = set_value(&file, &["model", "effort"], Value::String("extreme".into()));
    assert!(
        matches!(bad_value, Err(ConfigError::Invalid { .. })),
        "{bad_value:?}"
    );
    let through_scalar = set_value(&file, &["model", "main", "x"], Value::Integer(1));
    assert!(matches!(through_scalar, Err(ConfigError::KeyPath { .. })));
    assert!(matches!(
        set_value(&file, &["schema"], Value::Integer(9)),
        Err(ConfigError::KeyPath { .. })
    ));
    assert!(matches!(
        set_value(&file, &[], Value::Integer(9)),
        Err(ConfigError::KeyPath { .. })
    ));
    assert_eq!(read(&file), before);

    write(&file, "schema = 2\n[model\n");
    let unparseable = set_value(&file, &["model", "main"], Value::String("x".into()));
    assert!(matches!(unparseable, Err(ConfigError::Parse { .. })));
    assert_eq!(read(&file), "schema = 2\n[model\n");
}

#[test]
fn removals_from_a_missing_file_do_not_create_it() {
    let f = fixture();
    let file = project_local_file(&f.project);
    remove_value(&file, &["model", "main"]).unwrap();
    remove_from_array(&file, &["permissions", "allow"], "Read").unwrap();
    remove_mcp_server(&file, "fs").unwrap();
    assert!(!file.exists());
}

#[test]
fn a_v1_file_is_migrated_before_editing() {
    let f = fixture();
    let file = f.paths.user_config_file.clone();
    write(&file, "model = \"old\"\n[permissions]\nallow = [\"ls*\"]\n");
    add_permission_rule(&file, RuleKind::Allow, "Read").unwrap();
    assert_eq!(
        read(&f.paths.config_dir.join("config.v1.toml")),
        "model = \"old\"\n[permissions]\nallow = [\"ls*\"]\n"
    );
    let settings = f.load().settings;
    assert_eq!(settings.model.main, "old");
    assert_eq!(settings.permissions.allow, ["Bash(ls:*)", "Read"]);
}
