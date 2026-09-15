use std::path::Path;

use super::ProfileBuilder;

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    if text.trim().is_empty() {
        profile.warning(
            path,
            "empty_gradle_file",
            "Empty Gradle marker; no verification tasks were discovered.",
        );
    } else {
        profile.warning(path, "dynamic_configuration", "Gradle Groovy/Kotlin DSL is executable configuration. Plugins, applied scripts, and build/check/test tasks are not evaluated or assumed to exist.");
    }
}
