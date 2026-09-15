use std::collections::BTreeSet;
use std::path::Path;

use super::ProfileBuilder;
use crate::{CommandBasis, VerificationKind};

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    if text.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with(".RECIPEPREFIX") || line.starts_with(".if")
    }) {
        profile.warning(path, "make_dialect_unknown", "Custom recipe prefixes or non-GNU conditional syntax require manual inspection; targets were not guessed.");
        return;
    }
    let mut targets = BTreeSet::new();
    let mut conditional = 0usize;
    let mut continued = false;
    for line in text.lines() {
        let skip = continued;
        continued = line.trim_end().ends_with('\\');
        if skip || line.starts_with('\t') || line.trim_start().starts_with('#') {
            continue;
        }
        let directive = line.split_whitespace().next().unwrap_or("");
        if matches!(directive, "ifdef" | "ifndef" | "ifeq" | "ifneq" | "define") {
            conditional += 1;
            continue;
        }
        if matches!(directive, "endif" | "endef") {
            conditional = conditional.saturating_sub(1);
            continue;
        }
        if conditional != 0 || line.starts_with(char::is_whitespace) || continued {
            continue;
        }
        let Some((left, right)) = line.split_once(':') else {
            continue;
        };
        if right.starts_with('=') || left.contains(['$', '%', '=', '\\', '#']) {
            continue;
        }
        for target in left.split_whitespace() {
            if matches!(target, "all" | "build" | "check" | "test" | "lint") {
                targets.insert(target);
            }
        }
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    for target in targets {
        let kind = match target {
            "all" | "build" => VerificationKind::Build,
            "test" => VerificationKind::Test,
            "lint" => VerificationKind::Lint,
            _ => VerificationKind::Check,
        };
        profile.command(
            kind,
            "make",
            &["-f", &name, target],
            path,
            CommandBasis::DeclaredTarget,
        );
    }
    profile.warning(path, "static_parse_only", "Only literal, unconditional verification targets are recognized. Includes, variables, recipes, and Make dialect semantics are not evaluated.");
}
