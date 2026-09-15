use std::collections::BTreeSet;
use std::path::Path;

use super::{ProfileBuilder, xml};
use crate::{CommandBasis, VerificationKind};

pub(crate) fn parse(
    profile: &mut ProfileBuilder,
    path: &Path,
    text: &str,
    files: &BTreeSet<String>,
) {
    let elements = match xml::elements(text) {
        Ok(elements) => elements,
        Err(error) => return profile.malformed(path, error),
    };
    if elements[0].name != "project"
        || !elements
            .iter()
            .any(|e| e.depth == 1 && e.name == "modelVersion" && !e.text.trim().is_empty())
        || !elements
            .iter()
            .any(|e| e.depth == 1 && e.name == "artifactId" && !e.text.trim().is_empty())
    {
        return profile.malformed(
            path,
            "pom.xml needs a <project> root with modelVersion and artifactId.",
        );
    }
    let wrapper = if cfg!(windows) { "mvnw.cmd" } else { "mvnw" };
    let program = if files.contains(wrapper) {
        profile
            .profile
            .root
            .join(wrapper)
            .to_string_lossy()
            .into_owned()
    } else {
        "mvn".into()
    };
    for (kind, goal) in [
        (VerificationKind::Build, "compile"),
        (VerificationKind::Check, "verify"),
        (VerificationKind::Test, "test"),
    ] {
        profile.command(kind, &program, &[goal], path, CommandBasis::ToolConvention);
    }
}
