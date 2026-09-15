use std::path::Path;

use super::ProfileBuilder;
use crate::{CommandBasis, VerificationKind};

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    let modules: Vec<_> = text
        .lines()
        .map(|line| line.split("//").next().unwrap_or("").trim())
        .filter_map(|line| {
            line.strip_prefix("module")
                .filter(|s| s.starts_with(char::is_whitespace))
        })
        .collect();
    if modules.len() != 1 || !valid_module(modules[0].trim()) {
        return profile.malformed(
            path,
            "go.mod needs exactly one `module <module-path>` declaration.",
        );
    }
    profile.profile.languages.push("Go".into());
    for (kind, verb) in [
        (VerificationKind::Build, "build"),
        (VerificationKind::Check, "vet"),
        (VerificationKind::Test, "test"),
    ] {
        profile.command(
            kind,
            "go",
            &[verb, "./..."],
            path,
            CommandBasis::ToolConvention,
        );
    }
    profile.warning(path, "static_parse_only", "Only the module declaration was validated; Go directives and toolchain availability were not checked.");
}

fn valid_module(value: &str) -> bool {
    let path = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    !path.is_empty()
        && !path
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '`')
}
