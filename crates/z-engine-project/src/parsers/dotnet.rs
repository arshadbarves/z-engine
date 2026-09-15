use std::path::Path;

use super::{ProfileBuilder, xml};
use crate::{CommandBasis, VerificationKind};

pub(crate) fn parse(profile: &mut ProfileBuilder, path: &Path, text: &str) {
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let target = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if extension == "sln" {
        if !text
            .trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with("Microsoft Visual Studio Solution File, Format Version ")
        {
            return profile.malformed(path, "Expected a Visual Studio solution header.");
        }
        profile.command(
            VerificationKind::Build,
            "dotnet",
            &["build", &target],
            path,
            CommandBasis::ToolConvention,
        );
        profile.warning(path, "solution_not_resolved", "Solution references are not followed; included projects, test projects, and SDK compatibility are unknown.");
        return;
    }
    let elements = match xml::elements(text) {
        Ok(elements) => elements,
        Err(error) => return profile.malformed(path, error),
    };
    if extension == "slnx" {
        if elements[0].name != "Solution" {
            return profile.malformed(path, "Expected a <Solution> root.");
        }
        profile.command(
            VerificationKind::Build,
            "dotnet",
            &["build", &target],
            path,
            CommandBasis::ToolConvention,
        );
        profile.warning(
            path,
            "solution_not_resolved",
            "Solution references and installed SDK support for slnx were not checked.",
        );
        return;
    }
    if elements[0].name != "Project" {
        return profile.malformed(path, "Expected an MSBuild <Project> root.");
    }
    let sdk = elements[0].attributes.get("Sdk");
    let sdk_project = sdk.is_some_and(|sdk| !sdk.is_empty())
        || elements.iter().any(|element| element.name == "Sdk");
    if !sdk_project {
        profile.warning(path, "legacy_msbuild", "Non-SDK MSBuild project detected. Required MSBuild/Visual Studio toolchain and verification commands are unknown.");
        return;
    }
    profile.profile.languages.push(
        match extension {
            "csproj" => "C#",
            "fsproj" => "F#",
            "vbproj" => "Visual Basic",
            _ => return,
        }
        .into(),
    );
    profile.command(
        VerificationKind::Build,
        "dotnet",
        &["build", &target],
        path,
        CommandBasis::ToolConvention,
    );
    let test_project = sdk.is_some_and(|sdk| sdk.starts_with("MSTest.Sdk/"))
        || elements.iter().any(|element| {
            (element.name == "IsTestProject" && element.text.trim().eq_ignore_ascii_case("true"))
                || (element.name == "PackageReference"
                    && element
                        .attributes
                        .get("Include")
                        .is_some_and(|name| name == "Microsoft.NET.Test.Sdk"))
        });
    if test_project {
        profile.command(
            VerificationKind::Test,
            "dotnet",
            &["test", &target],
            path,
            CommandBasis::ToolConvention,
        );
        profile.warning(path, "conditional_configuration", "Test configuration was found statically; MSBuild conditions, imports, and target overrides were not evaluated.");
    }
}
