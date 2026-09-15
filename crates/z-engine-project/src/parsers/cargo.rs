use std::path::Path;

use super::ProfileBuilder;
use crate::{CommandBasis, VerificationKind};

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    let manifest = match text.parse::<toml::Table>() {
        Ok(manifest) => manifest,
        Err(error) => return profile.malformed(path, error),
    };
    let package = manifest.get("package");
    let workspace = manifest.get("workspace");
    if package.is_none() && workspace.is_none() {
        return profile.malformed(path, "Cargo.toml needs a [package] or [workspace] table.");
    }
    for (name, value) in [("package", package), ("workspace", workspace)] {
        if value.is_some_and(|v| !v.is_table()) {
            return profile.malformed(path, format!("{name} must be a table."));
        }
    }
    if package.is_some_and(|package| {
        package
            .get("name")
            .and_then(toml::Value::as_str)
            .is_none_or(|s| s.is_empty())
    }) {
        return profile.malformed(path, "[package].name must be a non-empty string.");
    }
    for field in ["members", "default-members", "exclude"] {
        if workspace
            .and_then(|workspace| workspace.get(field))
            .is_some_and(|value| {
                !value
                    .as_array()
                    .is_some_and(|items| items.iter().all(toml::Value::is_str))
            })
        {
            return profile.malformed(
                path,
                format!("[workspace].{field} must be an array of strings."),
            );
        }
    }
    profile.profile.languages.push("Rust".into());
    for (kind, args) in [
        (VerificationKind::Check, ["check"]),
        (VerificationKind::Build, ["build"]),
        (VerificationKind::Test, ["test"]),
    ] {
        profile.command(kind, "cargo", &args, path, CommandBasis::ToolConvention);
    }
}
