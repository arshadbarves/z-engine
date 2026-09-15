use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::filesystem::{Workspace, check_cancel};
use crate::parsers::{self, ProfileBuilder};
use crate::traversal::{self, Inventory};
use crate::{
    Diagnostic, DiscoveryError, DiscoveryOptions, ExecutionStatus, ProjectKind, ProjectProfile,
    ProjectReport, SupportLevel, markers,
};

/// Inspect an authorized workspace. Does not read parent/global ignore configuration.
pub fn discover(
    workspace: &Path,
    options: &DiscoveryOptions,
) -> Result<ProjectReport, DiscoveryError> {
    discover_with_cancel(workspace, options, &AtomicBool::new(false))
}

/// Cooperative cancellation is checked between directory entries and manifest reads.
pub fn discover_with_cancel(
    workspace: &Path,
    options: &DiscoveryOptions,
    cancel: &AtomicBool,
) -> Result<ProjectReport, DiscoveryError> {
    options.validate()?;
    check_cancel(cancel)?;
    let workspace = Workspace::open(workspace)?;
    let mut inventory = traversal::scan(&workspace, options, cancel)?;
    let files: BTreeMap<_, _> = inventory
        .files
        .iter()
        .map(|(relative, names)| (workspace.root.join(relative), names.clone()))
        .collect();
    let groups = group_profiles(&files, options, &mut inventory, &workspace.root);
    let mut documents = BTreeMap::new();
    let mut failures = BTreeMap::new();
    for paths in groups.values() {
        for path in paths {
            check_cancel(cancel)?;
            let relative = path.strip_prefix(&workspace.root).map_err(|_| {
                DiscoveryError::InvalidOptions(
                    "Internal manifest path escaped the workspace".into(),
                )
            })?;
            match workspace.read_text(relative, options, &mut inventory.stats) {
                Ok(text) => {
                    documents.insert(path.clone(), text);
                }
                Err(diagnostic) => {
                    inventory.stats.complete = false;
                    failures.insert(path.clone(), diagnostic);
                }
            }
        }
    }
    let mut profiles = Vec::new();
    for ((root, kind), paths) in groups {
        check_cancel(cancel)?;
        let mut profile = ProfileBuilder::new(root.clone(), kind, paths.clone());
        for path in paths {
            if let Some(diagnostic) = failures.remove(&path) {
                profile.diagnostic(diagnostic);
                continue;
            }
            let Some(text) = documents.get(&path) else {
                profile.malformed(&path, "Manifest was not available in the bounded scan.");
                continue;
            };
            match kind {
                ProjectKind::Cargo => parsers::cargo::parse(&mut profile, &path, text),
                ProjectKind::Node => parsers::node::parse(
                    &mut profile,
                    &path,
                    text,
                    &workspace.root,
                    &files,
                    &documents,
                ),
                ProjectKind::Python => parsers::python::parse(&mut profile, &path, text),
                ProjectKind::Go => parsers::go::parse(&mut profile, &path, text),
                ProjectKind::Gradle => parsers::gradle::parse(&mut profile, &path, text),
                ProjectKind::Maven => {
                    parsers::maven::parse(&mut profile, &path, text, &files[&root])
                }
                ProjectKind::Dotnet => parsers::dotnet::parse(&mut profile, &path, text),
                ProjectKind::Cmake => parsers::cmake::parse(&mut profile, &path, text),
                ProjectKind::Make => parsers::make::parse(&mut profile, &path, text),
                ProjectKind::Unknown => {}
            }
        }
        profiles.push(profile.finish());
    }
    attach_languages(&workspace.root, &mut inventory, &mut profiles, options);
    let languages: BTreeSet<_> = profiles
        .iter()
        .flat_map(|p| p.languages.iter().cloned())
        .chain(inventory.sources.values().flatten().cloned())
        .collect();
    Ok(ProjectReport {
        workspace_root: workspace.root,
        languages: languages.into_iter().collect(),
        profiles,
        scan: inventory.stats,
        diagnostics: inventory.diagnostics,
        execution: ExecutionStatus::NotRun,
        limitations: vec![
            "Discovery is read-only. No command, installation, network request, toolchain probe, or test was executed.".into(),
            "Commands are untrusted suggestions with exact cwd and argv, not permission to execute them. Commands and repository wrappers may mutate files, run arbitrary code, or access the network if later approved.".into(),
            "Toolchain availability, dependency installation, environment variables, and prerequisites are unknown. No successful verification or task-completion evidence is provided.".into(),
            "No universal language semantics or semantic refactoring support is claimed. Detected languages are filename/manifest evidence, not proof that a profile verifies every language in its root.".into(),
            "Only recognized static manifests and source extensions are inspected. Dynamic build configuration, transitive manifest references, custom tools, generated projects, and unrecognized languages remain unknown.".into(),
            "Symlinks, VCS/dependency/build directories, and locally ignored paths are skipped. Parent/global ignore files are not read. Scans are bounded and are not atomic filesystem snapshots.".into(),
        ],
    })
}

fn group_profiles(
    files: &BTreeMap<PathBuf, BTreeSet<String>>,
    options: &DiscoveryOptions,
    inventory: &mut Inventory,
    workspace: &Path,
) -> BTreeMap<(PathBuf, ProjectKind), Vec<PathBuf>> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    let mut limited = false;
    let mut manifests_limited = false;
    for (root, names) in files {
        for name in names {
            let Some(kind) = markers::kind(name) else {
                continue;
            };
            let key = (root.clone(), kind);
            if !groups.contains_key(&key) && groups.len() >= options.max_profiles {
                limited = true;
                continue;
            }
            let manifests = groups.entry(key).or_default();
            if manifests.len() >= 32 {
                manifests_limited = true;
            } else {
                manifests.push(root.join(name));
            }
        }
    }
    if limited {
        profile_limit(inventory, workspace);
    }
    if manifests_limited {
        inventory.stats.complete = false;
        inventory.diagnostics.push(Diagnostic::warning(
            "manifest_limit", workspace,
            "Only the first 32 manifests per profile were inspected. Inspect omitted project files separately.",
        ));
    }
    groups
}

fn attach_languages(
    workspace: &Path,
    inventory: &mut Inventory,
    profiles: &mut Vec<ProjectProfile>,
    options: &DiscoveryOptions,
) {
    let mut unmatched = BTreeSet::new();
    for (relative, languages) in &inventory.sources {
        let directory = workspace.join(relative);
        let depth = profiles
            .iter()
            .filter(|p| directory.starts_with(&p.root))
            .map(|p| p.root.components().count())
            .max();
        if let Some(depth) = depth {
            for profile in profiles
                .iter_mut()
                .filter(|p| directory.starts_with(&p.root) && p.root.components().count() == depth)
            {
                profile.languages.extend(languages.iter().cloned());
                profile.languages.sort();
                profile.languages.dedup();
            }
        } else {
            unmatched.extend(languages.iter().cloned());
        }
    }
    if profiles.is_empty() || !unmatched.is_empty() {
        if profiles.len() >= options.max_profiles {
            profile_limit(inventory, workspace);
            return;
        }
        let mut profile = ProfileBuilder::new(workspace.into(), ProjectKind::Unknown, Vec::new());
        profile.profile.languages = unmatched.into_iter().collect();
        profile.profile.support = SupportLevel::Unsupported;
        profile.warning(workspace, "unsupported_project", "No recognized project manifest covers these source files (or the workspace is empty/partially scanned). Verification is unconfigured, not successful.");
        profiles.push(profile.finish());
    }
}

fn profile_limit(inventory: &mut Inventory, workspace: &Path) {
    inventory.stats.complete = false;
    if !inventory
        .diagnostics
        .iter()
        .any(|d| d.code == "profile_limit")
    {
        inventory.diagnostics.push(Diagnostic::warning(
            "profile_limit", workspace,
            "More project profiles exist than max_profiles permits. Increase the limit or inspect a smaller workspace.",
        ));
    }
}
