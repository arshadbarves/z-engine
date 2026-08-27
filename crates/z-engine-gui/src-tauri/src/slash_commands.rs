use crate::state::GuiState;
use std::path::{Path, PathBuf};

// ---- custom slash commands --------------------------------------------------

#[derive(serde::Serialize)]
pub(crate) struct SlashCommandInfo {
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
pub(crate) fn list_slash_commands(state: tauri::State<'_, GuiState>) -> Vec<SlashCommandInfo> {
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
pub(crate) fn read_slash_command(
    name: String,
    state: tauri::State<'_, GuiState>,
) -> Result<String, String> {
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
