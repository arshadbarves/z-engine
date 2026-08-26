//! Desktop shell (Tauri 2) wrapping the harness-core brain.
//!
//! Serving model (rebuilt from scratch): a minimal HTTP server bound to
//! 127.0.0.1:<random port> serves the built frontend from disk, and the
//! main window is created programmatically pointed at that http:// URL.
//! No alternate schemes, no config-relative asset resolution.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use harness_core::agent::{AgentHandle, EventRx, spawn_with_recorder};
use harness_core::config::{CliOverrides, Config};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Shared application state managed by Tauri.
#[derive(Default)]
struct GuiState {
    handle: Mutex<Option<AgentHandle>>,
    ctx: Mutex<Option<AppCtx>>,
    /// Model id the running agent was started with / hot-switched to.
    model: Mutex<String>,
}

#[derive(Clone)]
struct AppCtx {
    project_root: PathBuf,
}

fn resolve_api_key() -> Option<String> {
    if let Ok(k) = std::env::var("HARNESS_API_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Some(k);
        }
    }
    let path = dirs::home_dir()?
        .join(".config")
        .join("harness")
        .join("api-key");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn build_loop_config(cfg: &Config, project_root: &Path) -> harness_core::agent::LoopConfig {
    harness_core::agent::LoopConfig {
        model: cfg.model.clone(),
        base_url: cfg.base_url.clone(),
        api_key: resolve_api_key(),
        project_root: project_root.to_path_buf(),
        tmp_dir: std::env::temp_dir(),
        initial_allow_rules: cfg.permissions.allow.clone(),
        max_context_tokens: cfg.max_context_tokens,
        max_output_tokens: cfg.max_output_tokens,
        hooks: cfg.hooks.clone(),
        compact_at_percent: cfg.compact_at_percent,
        keep_recent_messages: 12,
        review_enabled: cfg.review_enabled,
        mcp_servers: cfg.mcp_servers.clone(),
        auto_allow_tools: vec![],
        initial_mode: harness_core::agent::PermissionMode::Normal,
    }
}

fn sessions_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("sessions")
}

// ---- workspaces (Codex-desktop style project roots) ------------------------

fn workspaces_file() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("workspaces.json")
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct WorkspacesFile {
    roots: Vec<PathBuf>,
}

fn load_workspaces() -> Vec<PathBuf> {
    std::fs::read_to_string(workspaces_file())
        .ok()
        .and_then(|t| serde_json::from_str::<WorkspacesFile>(&t).ok())
        .map(|w| w.roots)
        .unwrap_or_default()
}

fn save_workspaces(roots: &[PathBuf]) -> Result<(), String> {
    let path = workspaces_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Deduplicate, preserving order.
    let mut seen = Vec::new();
    for r in roots {
        if !seen.contains(r) {
            seen.push(r.clone());
        }
    }
    let text =
        serde_json::to_string_pretty(&WorkspacesFile { roots: seen }).map_err(|e| e.to_string())?;
    // Temp-file + rename so a crash mid-write cannot truncate the
    // workspace registry.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn forward_events(mut rx: EventRx, window: tauri::WebviewWindow) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let payload = serde_json::to_value(&ev).unwrap_or(serde_json::Value::Null);
            if window.emit("appEvent", payload).is_err() {
                break;
            }
        }
    });
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
fn frontend_ready() {
    eprintln!("[gui] frontend mounted");
    tracing::info!("frontend mounted");
}

#[tauri::command]
fn submit(
    text: String,
    images: Option<Vec<String>>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard
        .as_ref()
        .ok_or("agent not started")?
        .submit_with_images(text, images.unwrap_or_default());
    Ok(())
}

#[tauri::command]
fn abort(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.abort();
    Ok(())
}

#[tauri::command]
fn compact(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.compact();
    Ok(())
}

#[tauri::command]
fn notes(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.request_notes();
    Ok(())
}

#[tauri::command]
fn set_mode(mode: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    use harness_core::agent::PermissionMode;
    let m = match mode.as_str() {
        "accept-edits" | "auto-accept edits" => PermissionMode::AutoAcceptEdits,
        "plan" => PermissionMode::Plan,
        _ => PermissionMode::Normal,
    };
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.set_mode(m);
    Ok(())
}

#[tauri::command]
fn set_model(model: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard
        .as_ref()
        .ok_or("agent not started")?
        .set_model(model.clone());
    *state.model.lock().map_err(|_| "state poisoned")? = model;
    Ok(())
}

/// Current agent-facing configuration for UI chrome (model picker,
/// context meter, cost estimate, settings tabs).
#[tauri::command]
fn get_config(state: tauri::State<'_, GuiState>) -> Result<serde_json::Value, String> {
    let model = state.model.lock().map_err(|_| "state poisoned")?.clone();
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = ctx_guard.as_ref() else {
        return Err("not initialized".into());
    };
    let cfg =
        Config::load(&Default::default(), Some(&ctx.project_root)).map_err(|e| e.to_string())?;
    let pricing = cfg.pricing_for(&model).map(|p| {
        json!({
            "usdPerMtokInput": p.usd_per_mtok_input,
            "usdPerMtokOutput": p.usd_per_mtok_output,
        })
    });
    let mcp_servers: Vec<serde_json::Value> = cfg
        .mcp_servers
        .iter()
        .map(|s| json!({ "name": s.name, "command": s.command, "args": s.args }))
        .collect();
    let cost_overrides: serde_json::Map<String, serde_json::Value> = cfg
        .cost_overrides
        .iter()
        .map(|(m, p)| {
            (
                m.clone(),
                json!({
                    "usdPerMtokInput": p.usd_per_mtok_input,
                    "usdPerMtokOutput": p.usd_per_mtok_output,
                }),
            )
        })
        .collect();
    Ok(json!({
        "model": model,
        "maxContextTokens": cfg.max_context_tokens,
        "maxOutputTokens": cfg.max_output_tokens,
        "compactAtPercent": cfg.compact_at_percent,
        "baseUrl": cfg.base_url,
        "reviewEnabled": cfg.review_enabled,
        "pricing": pricing,
        "mcpServers": mcp_servers,
        "costOverrides": cost_overrides,
        "version": env!("CARGO_PKG_VERSION"),
        "projectName": ctx
            .project_root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ctx.project_root.to_string_lossy().into_owned()),
    }))
}

/// Settings → General: persist scalars into `.harness/config.toml` and
/// hot-apply the model to the running agent when one exists.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn save_general(
    model: Option<String>,
    base_url: Option<String>,
    max_context_tokens: Option<u32>,
    review: Option<bool>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let over = harness_core::config::GeneralOverrides {
        model: model.clone(),
        base_url,
        max_context_tokens,
        review_enabled: review,
    };
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    harness_core::config::persist_general(&ctx.project_root, &over).map_err(|e| e.to_string())?;

    if let Some(m) = model {
        if let Some(h) = state.handle.lock().map_err(|_| "state poisoned")?.as_ref() {
            h.set_model(m.clone());
        }
        *state.model.lock().map_err(|_| "state poisoned")? = m;
    }
    Ok(())
}

/// Settings → Cost: per-model USD/MTok override persisted to
/// `.harness/config.toml` under `[cost.overrides]`.
#[tauri::command]
fn set_cost_override(
    model: String,
    usd_per_mtok_input: f64,
    usd_per_mtok_output: f64,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    harness_core::config::set_cost_override(
        &ctx.project_root,
        &model,
        harness_core::context::cost::Pricing {
            usd_per_mtok_input,
            usd_per_mtok_output,
        },
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_cost_override(model: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    harness_core::config::remove_cost_override(&ctx.project_root, &model).map_err(|e| e.to_string())
}

/// Transcript replay for the sessions sidebar: parse a session JSONL into
/// its event list so the frontend can rebuild the chat history.
#[tauri::command]
fn read_session(path: String) -> Result<Vec<serde_json::Value>, String> {
    let events = harness_core::session::read_events(std::path::Path::new(&path))
        .map_err(|e| e.to_string())?;
    Ok(events
        .into_iter()
        .map(|e| serde_json::to_value(&e).unwrap_or(serde_json::Value::Null))
        .collect())
}

/// Resolved MCP server table for the Settings tab.
#[tauri::command]
fn list_mcp_servers(state: tauri::State<'_, GuiState>) -> Result<Vec<serde_json::Value>, String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let ctx = ctx_guard.as_ref().ok_or("not initialized")?;
    let cfg =
        Config::load(&Default::default(), Some(&ctx.project_root)).map_err(|e| e.to_string())?;
    Ok(cfg
        .mcp_servers
        .iter()
        .map(|s| json!({ "name": s.name, "command": s.command, "args": s.args }))
        .collect())
}

/// Settings → MCP Test button: spawn the server, handshake, tools/list.
/// Returns tool names; the connection is dropped afterwards.
#[tauri::command]
async fn test_mcp_server(name: String) -> Result<Vec<String>, String> {
    use harness_core::mcp::McpConnection;
    // Resolve the server definition from layered config.
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = Config::load(&Default::default(), Some(&project_root)).map_err(|e| e.to_string())?;
    let srv = cfg
        .mcp_servers
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("no mcp server named '{name}'"))?;
    let conn = McpConnection::new(&srv.name, &srv.command, &srv.args, &project_root);
    conn.ensure().await?;
    Ok(conn
        .list_tools()
        .await
        .into_iter()
        .map(|t| t.name)
        .collect())
}

const WALK_SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".harness",
    ".venv",
    "__pycache__",
];
const WALK_MAX_ENTRIES: usize = 20_000;
const FILE_MATCH_CAP: usize = 200;

/// `@file` picker backend: gitignore-lite project walk filtered by a
/// case-insensitive substring query on the relative path.
#[tauri::command]
fn list_project_files(
    query: String,
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    let ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let root = ctx_guard
        .as_ref()
        .map(|c| c.project_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let q = query.to_lowercase();
    let mut out = Vec::new();
    let mut visited = 0usize;
    walk_files(&root, &root, &q, &mut out, &mut visited);
    out.sort_by_key(|p| (p.matches('/').count(), p.clone()));
    out.truncate(FILE_MATCH_CAP);
    Ok(out)
}

fn walk_files(root: &Path, dir: &Path, q: &str, out: &mut Vec<String>, visited: &mut usize) {
    if *visited >= WALK_MAX_ENTRIES || out.len() >= FILE_MATCH_CAP * 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        *visited += 1;
        if *visited >= WALK_MAX_ENTRIES || out.len() >= FILE_MATCH_CAP * 4 {
            return;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            if !WALK_SKIP_DIRS.contains(&name_lossy.as_ref()) && !name_lossy.starts_with('.') {
                walk_files(root, &path, q, out, visited);
            }
            continue;
        }
        if name_lossy.starts_with('.') {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        if q.is_empty() || rel.to_lowercase().contains(q) {
            out.push(rel);
        }
    }
}

/// `! <cmd>` shell passthrough — executed locally, never touches the model.
#[tauri::command]
fn shell(cmd: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.shell(cmd);
    Ok(())
}

/// Rewind: restore files touched by the last checkpointed turn.
#[tauri::command]
fn revert_last_turn(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard
        .as_ref()
        .ok_or("agent not started")?
        .revert_last_turn();
    Ok(())
}

/// Per-message revert: undo every file change from run-turn `keep`
/// (the user message being reverted) and everything after it.
#[tauri::command]
fn revert_to_turn(keep: u64, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard
        .as_ref()
        .ok_or("agent not started")?
        .revert_to_turn(keep);
    Ok(())
}

/// Pick the reasoning effort for reasoning-capable models.
#[tauri::command]
fn set_reasoning_effort(
    effort: Option<String>,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard
        .as_ref()
        .ok_or("agent not started")?
        .set_reasoning_effort(effort);
    Ok(())
}

// ---- custom slash commands --------------------------------------------------

#[derive(serde::Serialize)]
struct SlashCommandInfo {
    name: String,
    source: String,
    description: String,
}

fn slash_dirs(project_root: &Path) -> Vec<(String, PathBuf)> {
    let mut dirs = vec![(
        "project".to_string(),
        project_root.join(".harness").join("commands"),
    )];
    if let Some(home) = dirs::home_dir() {
        dirs.push(("global".to_string(), home.join(".config/harness/commands")));
    }
    dirs
}

/// User-defined slash commands: markdown files whose stem is the command
/// name. Project commands shadow global ones.
#[tauri::command]
fn list_slash_commands(state: tauri::State<'_, GuiState>) -> Vec<SlashCommandInfo> {
    let mut out: Vec<SlashCommandInfo> = Vec::new();
    let root = state
        .ctx
        .lock()
        .ok()
        .and_then(|c| c.as_ref().map(|c| c.project_root.clone()));
    let Some(root) = root else { return out };
    for (source_label, dir) in slash_dirs(&root) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if out.iter().any(|c| c.name == name) {
                continue; // project shadows global
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let description = text
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("")
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(72)
                .collect::<String>();
            out.push(SlashCommandInfo {
                name: name.to_string(),
                source: source_label.clone(),
                description,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Full template body of a user-defined command, or an error when absent.
#[tauri::command]
fn read_slash_command(name: String, state: tauri::State<'_, GuiState>) -> Result<String, String> {
    // Reject anything that could escape the commands directory.
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("invalid command name".into());
    }
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    for (_, dir) in slash_dirs(&root) {
        let path = dir.join(format!("{name}.md"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Ok(text);
        }
    }
    Err(format!("unknown command /{name}"))
}

// ---- diff review (changed files vs HEAD) ------------------------------------

#[derive(serde::Serialize)]
struct ChangedFile {
    path: String,
    status: String,
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Working-tree changes (vs HEAD) for the review panel.
#[tauri::command]
fn list_changed_files(state: tauri::State<'_, GuiState>) -> Result<Vec<ChangedFile>, String> {
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    let porcelain = git(&root, &["status", "--porcelain=v1", "-z"])?;
    let mut out = Vec::new();
    // -z output is NUL-separated: XY<space>path\0[orig\0]
    let mut iter = porcelain.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = iter.next() {
        let mut chars = entry.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let rest = chars.as_str();
        let status = if x != ' ' { x } else { y };
        let path = rest.trim_start().to_string();
        // Renames carry "new\0old\0"; keep the new side only.
        if x == 'R' || y == 'R' {
            iter.next();
        }
        let status = match status {
            '?' => "untracked",
            'A' => "added",
            'D' => "deleted",
            'M' | 'C' => "modified",
            'R' => "renamed",
            other => {
                let _ = other;
                "modified"
            }
        };
        out.push(ChangedFile {
            path,
            status: status.to_string(),
        });
    }
    Ok(out)
}

/// Unified diff of one file against HEAD (full content for untracked).
#[tauri::command]
fn diff_for_file(path: String, state: tauri::State<'_, GuiState>) -> Result<String, String> {
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    // The path is workspace-relative; canonicalize and require it to stay
    // inside the workspace (`..`, absolute paths and symlink escapes all
    // resolve to something that must still be under root).
    if Path::new(&path).is_absolute() {
        return Err("path must be relative to the workspace".into());
    }
    let resolved = std::fs::canonicalize(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("path escapes the workspace: {path}"));
    }
    let tracked = git(&root, &["ls-files", "--error-unmatch", &path]).is_ok();
    if tracked {
        git(&root, &["diff", "HEAD", "--", &path])
    } else {
        let content = std::fs::read_to_string(&resolved).unwrap_or_default();
        let mut out = format!("--- /dev/null\n+++ b/{path}\n");
        for line in content.lines() {
            out.push_str(&format!("+{line}\n"));
        }
        Ok(out)
    }
}

// ---- git worktrees ------------------------------------------------------------

/// Create a linked worktree under `.harness/worktrees/<name>` on its own
/// branch, keep it out of `git status`, and register it as a workspace.
#[tauri::command]
fn create_worktree(name: String, state: tauri::State<'_, GuiState>) -> Result<String, String> {
    let slug: String = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if slug.is_empty() {
        return Err("worktree name must contain letters or digits".into());
    }
    let root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();
    let rel = format!(".harness/worktrees/{slug}");
    git(
        &root,
        &["worktree", "add", &rel, "-b", &format!("harness/{slug}")],
    )
    .map_err(|e| format!("git worktree add failed: {e}"))?;

    // Keep the worktree invisible to `git status`.
    let exclude = root.join(".git/info/exclude");
    let line = format!("/{rel}");
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    if !existing.lines().any(|l| l.trim() == line) {
        // Guard the separator: appending onto a file without a trailing
        // newline would glue the entry onto the last existing line.
        let mut sep = "";
        if !existing.is_empty() && !existing.ends_with('\n') {
            sep = "\n";
        }
        let _ = std::fs::write(&exclude, format!("{existing}{sep}{line}\n"));
    }

    let abs = std::fs::canonicalize(root.join(&rel)).map_err(|e| e.to_string())?;
    let abs_str = abs.to_string_lossy().into_owned();
    let mut roots = load_workspaces();
    if !roots.contains(&abs) {
        roots.push(abs);
        save_workspaces(&roots)?;
    }
    Ok(abs_str)
}

#[tauri::command]
fn approve_with_rule(
    id: u64,
    decision: String,
    rule: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    use harness_core::agent::ApprovalDecision;
    let d = match decision.as_str() {
        "session" => ApprovalDecision::AlwaysSession { rule },
        "persist" => ApprovalDecision::AlwaysPersist { rule },
        _ => ApprovalDecision::Once,
    };
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.approve(id, d);
    Ok(())
}

#[tauri::command]
fn deny(id: u64, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.handle.lock().map_err(|_| "state poisoned")?;
    guard.as_ref().ok_or("agent not started")?.deny(id);
    Ok(())
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub path: String,
    pub ulid: String,
    pub first_user_msg: Option<String>,
    pub modified_ms: u64,
    pub project_root: Option<String>,
}

/// Defense-in-depth for IPC commands that take filesystem paths from the
/// webview: canonicalize and require the result to stay under `base`.
/// A compromised webview must not be able to delete or read arbitrary
/// files by passing `..` segments or symlinks pointing elsewhere.
fn contain(base: &Path, candidate: &str) -> Result<PathBuf, String> {
    let joined = base.join(candidate);
    let canon = std::fs::canonicalize(&joined).map_err(|e| format!("{}: {e}", joined.display()))?;
    let base_canon = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    if !canon.starts_with(&base_canon) {
        return Err(format!("path escapes the session store: {candidate}"));
    }
    Ok(canon)
}

#[tauri::command]
fn list_sessions() -> Result<Vec<SessionEntry>, String> {
    use std::time::UNIX_EPOCH;
    Ok(harness_core::session::list_sessions(&sessions_dir())
        .into_iter()
        .map(|s| SessionEntry {
            modified_ms: s
                .modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            path: s.path.to_string_lossy().into_owned(),
            ulid: s.ulid,
            first_user_msg: s.first_user_msg,
            project_root: s.project_root,
        })
        .collect())
}

#[tauri::command]
fn delete_session(path: String) -> Result<(), String> {
    let contained = contain(&sessions_dir(), &path)?;
    harness_core::session::delete_session(&contained).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_workspaces() -> Vec<String> {
    load_workspaces()
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Register a folder as a workspace (Codex "Open folder"). The path must
/// be an existing directory; duplicates are ignored. Returns the
/// canonical path actually stored.
#[tauri::command]
fn add_workspace(path: String) -> Result<String, String> {
    let canonical = std::fs::canonicalize(&path).map_err(|e| format!("{path}: {e}"))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    let mut roots = load_workspaces();
    if !roots.contains(&canonical) {
        roots.push(canonical.clone());
        save_workspaces(&roots)?;
    }
    Ok(canonical.to_string_lossy().into_owned())
}

#[tauri::command]
fn remove_workspace(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    let mut roots: Vec<PathBuf> = load_workspaces()
        .into_iter()
        .filter(|r| *r != target)
        .collect();
    // Also drop entries that refer to the same dir through a different path.
    if let Ok(canon) = std::fs::canonicalize(&target) {
        roots.retain(|r| *r != canon);
    }
    save_workspaces(&roots)
}

// ---- model catalog (models.dev + local overrides) ---------------------------

/// Trimmed model entry for the picker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CatalogModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    reasoning: bool,
    /// Vision / image input support.
    #[serde(default)]
    attachment: bool,
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CatalogProvider {
    #[serde(default)]
    name: String,
    #[serde(default)]
    models: BTreeMap<String, CatalogModel>,
}

type Catalog = BTreeMap<String, CatalogProvider>;

fn catalog_cache_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("models-cache.json")
}

/// Local override file in the same shape as the command output:
/// `{"providers": {"<id>": {"name": ..., "models": {"<id>": {...}}}}}`.
/// Entries are merged over the fetched catalog (fields win individually).
fn local_models_override() -> Catalog {
    let path = dirs::home_dir()
        .map(|h| h.join(".config/harness/models.json"))
        .unwrap_or_else(|| PathBuf::from("/tmp/harness-models.json"));
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Catalog>(&t).ok())
        .unwrap_or_default()
}

/// Fetch the models.dev catalog, trim to picker essentials, merge local
/// overrides, and cache on disk. Falls back to the stale cache (or just
/// the overrides) when offline.
#[tauri::command]
async fn fetch_model_catalog() -> Result<serde_json::Value, String> {
    const URL: &str = "https://models.dev/api.json";
    let cache = catalog_cache_path();
    let stale_ok = std::fs::metadata(&cache)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| {
            t.elapsed()
                .map(|e| e.as_secs() < 24 * 3600)
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let fetched: Option<Catalog> = match reqwest::Client::new()
        .get(URL)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(raw) => Some(trim_catalog(&raw)),
            Err(e) => {
                tracing::warn!(error = %e, "models.dev response parse failed");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "models.dev fetch failed");
            None
        }
    };

    if let Some(cat) = &fetched {
        if let Ok(text) = serde_json::to_string(cat) {
            if let Some(parent) = cache.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cache, text);
        }
    }

    let mut merged: Catalog = match (&fetched, stale_ok) {
        (Some(c), _) => c.clone(),
        (None, true) => serde_json::from_str(&std::fs::read_to_string(&cache).unwrap_or_default())
            .unwrap_or_default(),
        _ => BTreeMap::new(),
    };
    for (pid, prov) in local_models_override() {
        let entry = merged.entry(pid).or_insert(CatalogProvider {
            name: prov.name.clone(),
            models: BTreeMap::new(),
        });
        if !prov.name.is_empty() {
            entry.name = prov.name;
        }
        for (mid, model) in prov.models {
            entry.models.insert(mid, model);
        }
    }
    serde_json::to_value(&merged).map_err(|e| e.to_string())
}

/// Reduce the raw 4MB models.dev payload to what the picker shows.
fn trim_catalog(raw: &serde_json::Value) -> Catalog {
    let mut out = Catalog::new();
    let Some(providers) = raw.as_object() else {
        return out;
    };
    for (pid, pv) in providers {
        let name = pv.get("name").and_then(|v| v.as_str()).unwrap_or(pid);
        let provider = out.entry(pid.clone()).or_insert_with(|| CatalogProvider {
            name: name.to_string(),
            models: BTreeMap::new(),
        });
        if provider.name.is_empty() {
            provider.name = name.to_string();
        }
        let Some(models) = pv.get("models").and_then(|v| v.as_object()) else {
            continue;
        };
        for (mid, mv) in models {
            let limit = mv.get("limit");
            provider.models.insert(
                mid.clone(),
                CatalogModel {
                    name: mv
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(mid)
                        .to_string(),
                    reasoning: mv
                        .get("reasoning")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    attachment: mv
                        .get("attachment")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    context: limit
                        .and_then(|l| l.get("context"))
                        .and_then(|v| v.as_u64()),
                    output: limit.and_then(|l| l.get("output")).and_then(|v| v.as_u64()),
                },
            );
        }
    }
    out
}

#[tauri::command]
fn list_permission_rules(state: tauri::State<'_, GuiState>) -> Result<Vec<String>, String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    harness_core::config::list_bash_rules(&ctx.project_root).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_permission_rule(rule: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    harness_core::config::persist_bash_rule(&ctx.project_root, &rule)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_permission_rule(rule: String, state: tauri::State<'_, GuiState>) -> Result<(), String> {
    let guard = state.ctx.lock().map_err(|_| "state poisoned")?;
    let Some(ctx) = guard.as_ref() else {
        return Err("not initialized".into());
    };
    harness_core::config::remove_bash_rule(&ctx.project_root, &rule).map_err(|e| e.to_string())
}

#[tauri::command]
fn start_session(
    resume_path: Option<String>,
    root: Option<String>,
    state: tauri::State<'_, GuiState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Snapshot the current workspace root, then release the ctx lock so
    // the whole swap below runs under a single handle-lock section —
    // concurrent "New task" clicks must not interleave shutdown/spawn.
    let base_root = state
        .ctx
        .lock()
        .map_err(|_| "state poisoned")?
        .as_ref()
        .ok_or("not initialized")?
        .project_root
        .clone();

    // Per-workspace sessions run against the chosen root (Codex-style
    // projects); tools, config layering and @-file listing follow it.
    let project_root: PathBuf = match &root {
        Some(r) => {
            let canonical = std::fs::canonicalize(r).map_err(|e| format!("{r}: {e}"))?;
            if !canonical.is_dir() {
                return Err(format!("{} is not a directory", canonical.display()));
            }
            canonical
        }
        None => base_root,
    };
    let cfg = Config::load(&Default::default(), Some(&project_root)).map_err(|e| e.to_string())?;
    let lc = build_loop_config(&cfg, &project_root);

    let recorder: Option<harness_core::session::SessionWriter>;
    let recorder_path: Option<PathBuf>;
    let resume_state;
    match &resume_path {
        Some(p) => {
            // Only transcripts from our own session store may be resumed.
            let contained = contain(&sessions_dir(), p)?;
            let events =
                harness_core::session::read_events(&contained).map_err(|e| e.to_string())?;
            let replayed = harness_core::session::replay(&events);
            resume_state = Some(harness_core::agent::ResumeState {
                working: replayed.working,
                note_payloads: replayed.notes_replayed,
            });
            let w = harness_core::session::SessionWriter::append_to(&contained)
                .map_err(|e| e.to_string())?;
            recorder_path = Some(w.path.clone());
            recorder = Some(w);
        }
        None => {
            resume_state = None;
            let mut w = harness_core::session::SessionWriter::create(&sessions_dir())
                .map_err(|e| e.to_string())?;
            // Record the environment up front — the sidebar groups sessions
            // under their workspace via this Meta event's project_root.
            let _ = w.record(&harness_core::session::SessionEvent::Meta {
                model: lc.model.clone(),
                project_root: project_root.to_string_lossy().into_owned(),
            });
            recorder_path = Some(w.path.clone());
            recorder = Some(w);
        }
    }

    // Critical section: shutdown of the previous agent and publication of
    // the new one happen atomically from other commands' point of view.
    let (_handle, ev_rx) = {
        let mut handle_guard = state.handle.lock().map_err(|_| "state poisoned")?;
        if let Some(h) = handle_guard.as_ref() {
            h.shutdown();
        }
        let spawned = spawn_with_recorder(lc, resume_state, recorder);
        *handle_guard = Some(spawned.0.clone());
        spawned
    };

    // Follow the chosen workspace for @-files, config and rule persistence.
    if root.is_some() {
        let mut ctx_guard = state.ctx.lock().map_err(|_| "state poisoned")?;
        if let Some(c) = ctx_guard.as_mut() {
            c.project_root = project_root;
        }
    }
    *state.model.lock().map_err(|_| "state poisoned")? = cfg.model.clone();

    let window = app
        .get_webview_window("main")
        .ok_or("main window missing")?;
    forward_events(ev_rx, window);

    let ulid = recorder_path
        .as_ref()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    app.emit("sessionChanged", json!({ "ulid": ulid }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- local HTTP asset server ----------------------------------------------

fn main() {
    // App-lifetime tokio runtime entered on the main thread so agent
    // startup `tokio::spawn`s land on a real reactor under Tauri.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _enter = rt.enter(); // intentionally lives for the process

    // Log file lives under <data_dir>/harness/; the directory may not
    // exist on first run, and a logging failure must never block launch.
    let log_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness/harness-gui.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_writer(std::sync::Mutex::new(file))
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .without_time()
                .try_init()
                .ok();
        }
        Err(e) => {
            eprintln!("harness-gui: cannot open log {}: {e}", log_path.display());
        }
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GuiState::default())
        .invoke_handler(tauri::generate_handler![
            frontend_ready,
            submit,
            abort,
            compact,
            notes,
            set_mode,
            set_model,
            approve_with_rule,
            deny,
            list_sessions,
            delete_session,
            list_workspaces,
            add_workspace,
            remove_workspace,
            fetch_model_catalog,
            set_reasoning_effort,
            list_slash_commands,
            read_slash_command,
            list_changed_files,
            diff_for_file,
            create_worktree,
            list_permission_rules,
            save_permission_rule,
            remove_permission_rule,
            read_session,
            save_general,
            set_cost_override,
            remove_cost_override,
            list_mcp_servers,
            test_mcp_server,
            list_project_files,
            get_config,
            shell,
            revert_last_turn,
            revert_to_turn,
            start_session
        ])
        .setup(|app| {
            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let cfg = Config::load(&CliOverrides::default(), Some(&project_root))
                .map_err(|e| e.to_string())?;
            let lc = build_loop_config(&cfg, &project_root);

            let (handle, ev_rx) = spawn_with_recorder(lc, None, None);
            {
                let st = app.state::<GuiState>();
                *st.handle.lock().unwrap() = Some(handle);
                *st.ctx.lock().unwrap() = Some(AppCtx {
                    project_root: project_root.clone(),
                });
                *st.model.lock().unwrap() = cfg.model.clone();
            }

            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("harness")
            .inner_size(1100.0, 760.0)
            .min_inner_size(720.0, 520.0)
            // Codex-desktop chrome: no separate title bar — traffic lights
            // float over the sidebar (which pads for them).
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .build()
            .map_err(|e| e.to_string())?;

            forward_events(ev_rx, window);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building harness GUI");
    app.run(|_app_handle, event| {
        // Tear the agent down on quit so bash/MCP child processes don't
        // outlive the closed window as orphans.
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            if let Some(st) = _app_handle.try_state::<GuiState>() {
                if let Ok(mut guard) = st.handle.lock() {
                    if let Some(h) = guard.take() {
                        h.shutdown();
                    }
                }
            }
        }
    });
}
