use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::filesystem::{Workspace, check_cancel};
use crate::{Diagnostic, DiscoveryError, DiscoveryOptions, ScanStats, markers};

#[derive(Default)]
pub(crate) struct Inventory {
    pub files: BTreeMap<PathBuf, BTreeSet<String>>,
    pub sources: BTreeMap<PathBuf, BTreeSet<String>>,
    pub stats: ScanStats,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn scan(
    workspace: &Workspace,
    options: &DiscoveryOptions,
    cancel: &AtomicBool,
) -> Result<Inventory, DiscoveryError> {
    let mut inventory = Inventory::default();
    inventory.stats.complete = true;
    let mut pending = VecDeque::from([(PathBuf::new(), 0, Vec::<Arc<Gitignore>>::new())]);
    while let Some((relative, depth, mut ignores)) = pending.pop_front() {
        check_cancel(cancel)?;
        if inventory.stats.entries_seen >= options.max_entries {
            limit(
                &mut inventory,
                &workspace.root,
                "entry_limit",
                "max_entries",
            );
            break;
        }
        if !load_ignores(workspace, &relative, options, &mut inventory, &mut ignores) {
            continue;
        }
        let directory = match workspace
            .directory
            .open_dir(if relative.as_os_str().is_empty() {
                Path::new(".")
            } else {
                &relative
            }) {
            Ok(dir) => dir,
            Err(error) => {
                io_error(&mut inventory, workspace.root.join(&relative), error);
                continue;
            }
        };
        let entries = match directory.entries() {
            Ok(entries) => entries,
            Err(error) => {
                io_error(&mut inventory, workspace.root.join(&relative), error);
                continue;
            }
        };
        let mut children = Vec::new();
        for entry in entries {
            check_cancel(cancel)?;
            if inventory.stats.entries_seen >= options.max_entries {
                limit(
                    &mut inventory,
                    &workspace.root,
                    "entry_limit",
                    "max_entries",
                );
                break;
            }
            inventory.stats.entries_seen += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    io_error(&mut inventory, workspace.root.join(&relative), error);
                    continue;
                }
            };
            let path = relative.join(entry.file_name());
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    io_error(&mut inventory, workspace.root.join(path), error);
                    continue;
                }
            };
            if file_type.is_symlink() {
                inventory.stats.skipped_symlinks += 1;
                continue;
            }
            if !file_type.is_file() && !file_type.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if (file_type.is_dir() && markers::excluded_directory(&name))
                || name == ".git"
                || is_ignored(&ignores, &workspace.root.join(&path), file_type.is_dir())
            {
                inventory.stats.skipped_ignored += 1;
                continue;
            }
            if file_type.is_dir() {
                if depth >= options.max_depth {
                    limit(&mut inventory, &workspace.root, "depth_limit", "max_depth");
                } else {
                    children.push(path);
                }
            } else {
                if markers::kind(&name).is_some() || markers::marker_only(&name) {
                    inventory
                        .files
                        .entry(relative.clone())
                        .or_default()
                        .insert(name);
                }
                if let Some(language) = markers::language(&path) {
                    inventory
                        .sources
                        .entry(relative.clone())
                        .or_default()
                        .insert(language.into());
                }
            }
        }
        children.sort();
        pending.extend(
            children
                .into_iter()
                .map(|path| (path, depth + 1, ignores.clone())),
        );
    }
    Ok(inventory)
}

fn load_ignores(
    workspace: &Workspace,
    relative: &Path,
    options: &DiscoveryOptions,
    inventory: &mut Inventory,
    ignores: &mut Vec<Arc<Gitignore>>,
) -> bool {
    for name in [".gitignore", ".ignore"] {
        let path = relative.join(name);
        match workspace.directory.symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => continue,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                io_error(inventory, workspace.root.join(path), error);
                return false;
            }
        }
        let text = match workspace.read_text(&path, options, &mut inventory.stats) {
            Ok(text) => text,
            Err(diagnostic) => {
                inventory.stats.complete = false;
                inventory.diagnostics.push(diagnostic);
                return false;
            }
        };
        let mut builder = GitignoreBuilder::new(workspace.root.join(relative));
        for line in text.lines() {
            if let Err(error) = builder.add_line(Some(workspace.root.join(&path)), line) {
                inventory.stats.complete = false;
                inventory.diagnostics.push(Diagnostic::error(
                    "invalid_ignore",
                    workspace.root.join(&path),
                    format!("Invalid ignore pattern: {error}"),
                ));
                return false;
            }
        }
        match builder.build() {
            Ok(ignore) => ignores.push(Arc::new(ignore)),
            Err(error) => {
                inventory.stats.complete = false;
                inventory.diagnostics.push(Diagnostic::error(
                    "invalid_ignore",
                    workspace.root.join(&path),
                    format!("Cannot compile ignore patterns: {error}"),
                ));
                return false;
            }
        }
    }
    true
}

fn is_ignored(ignores: &[Arc<Gitignore>], path: &Path, is_dir: bool) -> bool {
    let mut ignored = false;
    for rules in ignores {
        let matched = rules.matched_path_or_any_parents(path, is_dir);
        if matched.is_ignore() {
            ignored = true;
        } else if matched.is_whitelist() {
            ignored = false;
        }
    }
    ignored
}

fn io_error(inventory: &mut Inventory, path: PathBuf, error: std::io::Error) {
    inventory.stats.complete = false;
    inventory.diagnostics.push(Diagnostic::error(
        "scan_failed",
        path,
        format!("Cannot inspect directory entry: {error}"),
    ));
}

fn limit(inventory: &mut Inventory, root: &Path, code: &str, option: &str) {
    inventory.stats.complete = false;
    if !inventory.diagnostics.iter().any(|d| d.code == code) {
        inventory.diagnostics.push(Diagnostic::warning(
            code, root, format!("Discovery reached {option}; the report is partial. Increase that limit or inspect a smaller workspace."),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_children_share_loaded_ignore_matchers() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(".gitignore"), "ignored/\n").unwrap();
        let workspace = Workspace::open(root.path()).unwrap();
        let mut ignores = Vec::new();
        assert!(load_ignores(
            &workspace,
            Path::new(""),
            &DiscoveryOptions::default(),
            &mut Inventory::default(),
            &mut ignores,
        ));
        let children: Vec<_> = (0..10_000).map(|_| ignores.clone()).collect();
        assert_eq!(Arc::strong_count(&ignores[0]), 10_001);
        assert!(
            children
                .iter()
                .all(|child| Arc::ptr_eq(&child[0], &ignores[0]))
        );
    }
}
