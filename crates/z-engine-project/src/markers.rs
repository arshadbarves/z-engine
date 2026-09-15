use std::path::Path;

use crate::ProjectKind;

pub(crate) fn kind(name: &str) -> Option<ProjectKind> {
    Some(match name {
        "Cargo.toml" => ProjectKind::Cargo,
        "package.json" => ProjectKind::Node,
        "pyproject.toml" | "pytest.ini" | "tox.ini" | "setup.cfg" => ProjectKind::Python,
        "go.mod" => ProjectKind::Go,
        "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
            ProjectKind::Gradle
        }
        "pom.xml" => ProjectKind::Maven,
        "CMakeLists.txt" | "CMakePresets.json" => ProjectKind::Cmake,
        "Makefile" | "makefile" | "GNUmakefile" => ProjectKind::Make,
        _ if matches!(
            Path::new(name).extension().and_then(|e| e.to_str()),
            Some("csproj" | "fsproj" | "vbproj" | "sln" | "slnx")
        ) =>
        {
            ProjectKind::Dotnet
        }
        _ => return None,
    })
}

pub(crate) fn marker_only(name: &str) -> bool {
    matches!(
        name,
        "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lock"
            | "bun.lockb"
            | "gradlew"
            | "gradlew.bat"
            | "mvnw"
            | "mvnw.cmd"
    )
}

pub(crate) fn excluded_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "out"
            | "vendor"
            | "bin"
            | "obj"
            | "coverage"
            | ".cache"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".tox"
            | ".nox"
            | ".mypy_cache"
            | ".pytest_cache"
            | ".ruff_cache"
            | ".gradle"
            | ".idea"
            | ".vs"
            | ".yarn"
            | ".pnpm-store"
            | "Pods"
            | ".dart_tool"
            | "CMakeFiles"
    ) || name.starts_with("cmake-build-")
}

pub(crate) fn language(path: &Path) -> Option<&'static str> {
    Some(match path.extension()?.to_str()? {
        "rs" => "Rust",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "py" | "pyi" => "Python",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "scala" => "Scala",
        "cs" => "C#",
        "fs" | "fsx" => "F#",
        "vb" => "Visual Basic",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hxx" => "C++",
        "swift" => "Swift",
        "m" | "mm" => "Objective-C",
        "rb" => "Ruby",
        "php" => "PHP",
        "ex" | "exs" => "Elixir",
        "erl" => "Erlang",
        "hs" => "Haskell",
        "lua" => "Lua",
        "clj" | "cljs" => "Clojure",
        "dart" => "Dart",
        "zig" => "Zig",
        "r" | "R" => "R",
        "pl" | "pm" => "Perl",
        "sql" => "SQL",
        _ => return None,
    })
}
