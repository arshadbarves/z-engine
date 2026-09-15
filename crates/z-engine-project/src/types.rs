use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Cargo,
    Node,
    Python,
    Go,
    Gradle,
    Maven,
    Dotnet,
    Cmake,
    Make,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    /// At least one statically justified command suggestion; never a toolchain probe.
    Discovered,
    /// A marker exists, but no reliable verification command was extracted.
    MarkerOnly,
    /// Source files exist without a recognized project format.
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationKind {
    Build,
    Check,
    Test,
    Lint,
    FormatCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandBasis {
    ToolConvention,
    DeclaredScript,
    DeclaredTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    NotRun,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandSuggestion {
    pub kind: VerificationKind,
    pub program: String,
    /// Argument vector, not shell text. Repository-provided values remain untrusted data.
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub evidence: Vec<PathBuf>,
    pub basis: CommandBasis,
    pub execution: ExecutionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    CommandsSuggested,
    Unconfigured,
    Unknown,
    NotProvided,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileCapabilities {
    pub build: CapabilityStatus,
    pub check: CapabilityStatus,
    pub test: CapabilityStatus,
    pub toolchain_availability: CapabilityStatus,
    pub semantic_refactoring: CapabilityStatus,
    pub verified_completion: CapabilityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub path: PathBuf,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn error(code: &str, path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }

    pub(crate) fn warning(
        code: &str,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectProfile {
    pub root: PathBuf,
    pub kind: ProjectKind,
    pub manifests: Vec<PathBuf>,
    pub languages: Vec<String>,
    pub support: SupportLevel,
    pub commands: Vec<CommandSuggestion>,
    pub capabilities: ProfileCapabilities,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ScanStats {
    pub entries_seen: usize,
    pub bytes_read: usize,
    pub skipped_symlinks: usize,
    pub skipped_ignored: usize,
    /// False for exhausted limits or unreadable input; exclusions still apply when true.
    pub complete: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectReport {
    pub workspace_root: PathBuf,
    pub languages: Vec<String>,
    pub profiles: Vec<ProjectProfile>,
    pub scan: ScanStats,
    pub diagnostics: Vec<Diagnostic>,
    pub execution: ExecutionStatus,
    pub limitations: Vec<String>,
}

impl ProjectReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .chain(self.profiles.iter().flat_map(|p| &p.diagnostics))
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }
}
