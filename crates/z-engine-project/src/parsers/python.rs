use std::collections::BTreeSet;
use std::path::Path;

use super::ProfileBuilder;
use crate::{CommandBasis, VerificationKind};

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    profile.profile.languages.push("Python".into());
    if path
        .file_name()
        .is_some_and(|name| name == "pyproject.toml")
    {
        parse_pyproject(profile, path, text);
    } else {
        parse_ini(profile, path, text);
    }
}

fn parse_pyproject(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    let manifest = match text.parse::<toml::Table>() {
        Ok(manifest) => manifest,
        Err(error) => return profile.malformed(path, error),
    };
    for key in ["project", "tool", "build-system"] {
        if manifest.get(key).is_some_and(|value| !value.is_table()) {
            return profile.malformed(path, format!("{key} must be a table."));
        }
    }
    let tool = manifest.get("tool").and_then(toml::Value::as_table);
    for key in ["pytest", "ruff", "mypy", "poetry"] {
        if tool
            .and_then(|tool| tool.get(key))
            .is_some_and(|value| !value.is_table())
        {
            return profile.malformed(path, format!("tool.{key} must be a table."));
        }
    }
    let pytest = tool.and_then(|tool| tool.get("pytest"));
    if let Some(options) = pytest.and_then(|pytest| pytest.get("ini_options")) {
        if !options.is_table() {
            return profile.malformed(path, "tool.pytest.ini_options must be a table.");
        }
        pytest_command(profile, path);
    }
    if tool.is_some_and(|tool| tool.contains_key("ruff")) {
        profile.command(
            VerificationKind::Lint,
            "python",
            &["-m", "ruff", "check", "."],
            path,
            CommandBasis::ToolConvention,
        );
    }
    if profile.profile.commands.is_empty() {
        profile.warning(path, "verification_unconfigured", "No supported pytest or Ruff configuration found. Build backends, dependencies, and entry-point scripts do not prove an available verification command.");
    }
}

fn parse_ini(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    let mut sections = BTreeSet::new();
    let mut section = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(['#', ';']) {
            continue;
        }
        if trimmed.starts_with('[') {
            let Some(end) = trimmed.find(']') else {
                return profile
                    .malformed(path, format!("Unclosed INI section at line {}.", index + 1));
            };
            let name = &trimmed[1..end];
            let suffix = trimmed[end + 1..].trim();
            if name.is_empty() || (!suffix.is_empty() && !suffix.starts_with(['#', ';'])) {
                return profile
                    .malformed(path, format!("Invalid INI section at line {}.", index + 1));
            }
            sections.insert(name.to_string());
            section = Some(name);
        } else if section.is_none()
            || (!line.starts_with(char::is_whitespace) && !trimmed.contains(['=', ':']))
        {
            return profile.malformed(path, format!("Invalid INI setting at line {}.", index + 1));
        }
    }
    let expected = if path.file_name().is_some_and(|name| name == "setup.cfg") {
        "tool:pytest"
    } else {
        "pytest"
    };
    if sections.contains(expected) {
        pytest_command(profile, path);
    } else if path.file_name().is_some_and(|name| name == "pytest.ini") {
        profile.malformed(path, "pytest.ini must contain a [pytest] section.");
    } else {
        profile.warning(path, "verification_unconfigured", "No pytest section found. Tox environments and arbitrary setup.cfg tooling are not evaluated.");
    }
}

fn pytest_command(profile: &mut ProfileBuilder, path: &Path) {
    if !profile
        .profile
        .commands
        .iter()
        .any(|command| command.kind == VerificationKind::Test)
    {
        profile.command(
            VerificationKind::Test,
            "python",
            &["-m", "pytest"],
            path,
            CommandBasis::ToolConvention,
        );
    }
}
