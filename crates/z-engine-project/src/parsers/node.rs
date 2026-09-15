use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::ProfileBuilder;
use crate::{CommandBasis, VerificationKind};

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    scripts: BTreeMap<String, String>,
    #[serde(rename = "packageManager")]
    package_manager: Option<String>,
}

pub(crate) fn parse(
    profile: &mut ProfileBuilder,
    path: &Path,
    text: &str,
    workspace: &Path,
    files: &BTreeMap<PathBuf, BTreeSet<String>>,
    documents: &BTreeMap<PathBuf, String>,
) {
    let manifest = match parse_manifest(text) {
        Ok(manifest) => manifest,
        Err(error) => return profile.malformed(path, error),
    };
    let Some((manager, evidence)) =
        select_manager(profile, path, &manifest, workspace, files, documents)
    else {
        return;
    };
    for (script, body) in &manifest.scripts {
        let Some(kind) = script_kind(script) else {
            continue;
        };
        if body.trim().is_empty() || body.contains("Error: no test specified") {
            profile.warning(path, "placeholder_script", format!("Script {script:?} is empty or the npm test placeholder; no usable check is configured."));
            continue;
        }
        if script.len() > 128 || script.chars().any(char::is_control) {
            profile.warning(
                path,
                "unsupported_script_name",
                "A verification script has an oversized or control-character name and was omitted.",
            );
            continue;
        }
        profile.command(
            kind,
            &manager,
            &["run", script],
            path,
            CommandBasis::DeclaredScript,
        );
        if let Some(command) = profile
            .profile
            .commands
            .last_mut()
            .filter(|command| evidence != path && !command.evidence.contains(&evidence))
        {
            command.evidence.push(evidence.clone());
        }
    }
    if profile.profile.commands.is_empty() {
        profile.warning(path, "verification_unconfigured", "No nonempty build/test/check/lint/typecheck/format-check scripts were declared. No script names were invented.");
    }
}

fn script_kind(name: &str) -> Option<VerificationKind> {
    let prefix = name.split(':').next()?;
    Some(match prefix {
        "build" => VerificationKind::Build,
        "test" => VerificationKind::Test,
        "check" | "typecheck" | "type-check" => VerificationKind::Check,
        "lint" if !name.split(':').any(|part| part == "fix") => VerificationKind::Lint,
        "format" | "fmt" if name.ends_with(":check") => VerificationKind::FormatCheck,
        _ => return None,
    })
}

fn select_manager(
    profile: &mut ProfileBuilder,
    path: &Path,
    manifest: &Manifest,
    workspace: &Path,
    files: &BTreeMap<PathBuf, BTreeSet<String>>,
    documents: &BTreeMap<PathBuf, String>,
) -> Option<(String, PathBuf)> {
    let root = profile.profile.root.clone();
    for directory in root
        .ancestors()
        .take_while(|dir| dir.starts_with(workspace))
    {
        let package = directory.join("package.json");
        let inherited;
        let current = if directory == root {
            Some(manifest)
        } else if let Some(text) = documents.get(&package) {
            inherited = match parse_manifest(text) {
                Ok(manifest) => manifest,
                Err(error) => {
                    profile.warning(&package, "invalid_package_manager_context", format!("Cannot infer inherited package manager from malformed package.json: {error}"));
                    return None;
                }
            };
            Some(&inherited)
        } else if files
            .get(directory)
            .is_some_and(|names| names.contains("package.json"))
        {
            profile.warning(
                &package,
                "unread_package_manager_context",
                "An ancestor manifest could not be read; package manager selection is unknown.",
            );
            return None;
        } else {
            None
        };
        let locks = lock_managers(directory, files.get(directory));
        if let Some(declared) = current.and_then(|m| m.package_manager.as_deref()) {
            let manager = declared.split('@').next().unwrap_or("");
            if !matches!(manager, "npm" | "pnpm" | "yarn" | "bun") {
                profile.warning(
                    &package,
                    "unsupported_package_manager",
                    format!(
                        "Unsupported packageManager value {declared:?}; commands were not guessed."
                    ),
                );
                return None;
            }
            if locks.keys().any(|lock| *lock != manager) {
                profile.warning(&package, "conflicting_lockfiles", "packageManager takes precedence over conflicting lockfiles; resolve stale lockfiles before executing.");
            }
            return Some((manager.into(), package));
        }
        if locks.len() > 1 {
            profile.warning(path, "ambiguous_package_manager", format!("Conflicting package-manager lockfiles in {}; set packageManager or remove stale locks.", directory.display()));
            return None;
        }
        if let Some((manager, lockfile)) = locks.into_iter().next() {
            return Some((manager.into(), lockfile));
        }
    }
    profile.warning(path, "package_manager_unconfigured", "No packageManager or lockfile found. npm is suggested by package.json convention only; availability is unknown.");
    Some(("npm".into(), path.into()))
}

fn parse_manifest(text: &str) -> Result<Manifest, serde_json::Error> {
    let object: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text)?;
    serde_json::from_value(serde_json::Value::Object(object))
}

fn lock_managers(
    directory: &Path,
    files: Option<&BTreeSet<String>>,
) -> BTreeMap<&'static str, PathBuf> {
    let mut managers = BTreeMap::new();
    if let Some(files) = files {
        for (name, manager) in [
            ("package-lock.json", "npm"),
            ("npm-shrinkwrap.json", "npm"),
            ("yarn.lock", "yarn"),
            ("pnpm-lock.yaml", "pnpm"),
            ("bun.lock", "bun"),
            ("bun.lockb", "bun"),
        ] {
            if files.contains(name) {
                managers.insert(manager, directory.join(name));
            }
        }
    }
    managers
}
