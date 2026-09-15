use std::path::Path;

use serde::Deserialize;

use super::ProfileBuilder;
use crate::{CommandBasis, VerificationKind};

#[derive(Deserialize)]
struct Presets {
    version: u64,
    #[serde(default, rename = "buildPresets")]
    builds: Vec<Preset>,
    #[serde(default, rename = "testPresets")]
    tests: Vec<Preset>,
}

#[derive(Deserialize)]
struct Preset {
    name: String,
    #[serde(default)]
    hidden: bool,
    condition: Option<serde_json::Value>,
    #[serde(rename = "configurePreset")]
    configure: Option<String>,
}

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    if path
        .file_name()
        .is_none_or(|name| name != "CMakePresets.json")
    {
        profile.warning(path, "cmake_unconfigured", "CMakeLists.txt found. No build directory, generator, or test registration is assumed; configure/build/test prerequisites need separate inspection.");
        return;
    }
    let object: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(text) {
        Ok(object) => object,
        Err(error) => return profile.malformed(path, error),
    };
    let presets: Presets = match serde_json::from_value(serde_json::Value::Object(object)) {
        Ok(presets) => presets,
        Err(error) => return profile.malformed(path, error),
    };
    if presets.version == 0
        || (presets.version < 2 && (!presets.builds.is_empty() || !presets.tests.is_empty()))
    {
        return profile.malformed(
            path,
            "Build/test presets require CMake preset schema version 2 or newer.",
        );
    }
    for (kind, candidates) in [
        (VerificationKind::Build, presets.builds),
        (VerificationKind::Test, presets.tests),
    ] {
        for preset in candidates {
            if preset.hidden || preset.condition.is_some() || preset.configure.is_none() {
                profile.warning(path, "unresolved_preset", "Hidden, conditional, or inherited-only presets are not suggested without resolving their configuration.");
                continue;
            }
            if preset.name.is_empty()
                || preset.name.len() > 128
                || preset.name.chars().any(char::is_control)
            {
                profile.malformed(path, "Preset names must be nonempty, at most 128 bytes, and contain no control characters.");
                continue;
            }
            if kind == VerificationKind::Build {
                profile.command(
                    kind,
                    "cmake",
                    &["--build", "--preset", &preset.name],
                    path,
                    CommandBasis::DeclaredTarget,
                );
            } else {
                profile.command(
                    kind,
                    "ctest",
                    &["--preset", &preset.name],
                    path,
                    CommandBasis::DeclaredTarget,
                );
            }
        }
    }
    profile.warning(path, "cmake_prerequisites_unknown", "Preset names are declared, not executed. Configure presets, includes, build directories, CMake version, and test registration were not resolved.");
}
