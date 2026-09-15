use std::path::{Path, PathBuf};

use crate::{
    CapabilityStatus, CommandBasis, CommandSuggestion, Diagnostic, DiagnosticSeverity,
    ExecutionStatus, ProfileCapabilities, ProjectKind, ProjectProfile, SupportLevel,
    VerificationKind,
};

pub(crate) struct ProfileBuilder {
    pub profile: ProjectProfile,
}

impl ProfileBuilder {
    pub fn new(root: PathBuf, kind: ProjectKind, manifests: Vec<PathBuf>) -> Self {
        Self {
            profile: ProjectProfile {
                root,
                kind,
                manifests,
                languages: Vec::new(),
                commands: Vec::new(),
                support: SupportLevel::MarkerOnly,
                capabilities: ProfileCapabilities {
                    build: CapabilityStatus::Unconfigured,
                    check: CapabilityStatus::Unconfigured,
                    test: CapabilityStatus::Unconfigured,
                    toolchain_availability: CapabilityStatus::Unknown,
                    semantic_refactoring: CapabilityStatus::Unknown,
                    verified_completion: CapabilityStatus::NotProvided,
                },
                diagnostics: Vec::new(),
            },
        }
    }

    pub fn command(
        &mut self,
        kind: VerificationKind,
        program: &str,
        args: &[&str],
        evidence: &Path,
        basis: CommandBasis,
    ) {
        if self.profile.commands.len() >= 32 {
            if !self
                .profile
                .diagnostics
                .iter()
                .any(|d| d.code == "command_limit")
            {
                self.warning(
                    evidence,
                    "command_limit",
                    "Only the first 32 command suggestions are reported.",
                );
            }
            return;
        }
        self.profile.commands.push(CommandSuggestion {
            kind,
            program: program.into(),
            args: args.iter().map(|s| (*s).into()).collect(),
            cwd: self.profile.root.clone(),
            evidence: vec![evidence.into()],
            basis,
            execution: ExecutionStatus::NotRun,
        });
    }

    pub fn warning(&mut self, path: &Path, code: &str, message: impl Into<String>) {
        self.diagnostic(Diagnostic::warning(code, path, message));
    }

    pub fn malformed(&mut self, path: &Path, message: impl std::fmt::Display) {
        self.diagnostic(Diagnostic::error(
            "malformed_manifest",
            path,
            format!("Fix this manifest and rescan: {message}"),
        ));
    }

    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        if self.profile.diagnostics.len() < 32 {
            self.profile.diagnostics.push(diagnostic);
        } else if let Some(limit) = self
            .profile
            .diagnostics
            .iter_mut()
            .find(|d| d.code == "diagnostic_limit")
        {
            if diagnostic.severity == DiagnosticSeverity::Error {
                limit.severity = DiagnosticSeverity::Error;
            }
        } else {
            let mut limit = Diagnostic::warning(
                "diagnostic_limit",
                &self.profile.root,
                "Additional profile diagnostics were omitted after 32 messages. Inspect the manifests directly.",
            );
            limit.severity = diagnostic.severity;
            self.profile.diagnostics.push(limit);
        }
    }

    pub fn finish(mut self) -> ProjectProfile {
        use CapabilityStatus::CommandsSuggested;
        if self
            .profile
            .diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error || d.code == "byte_limit")
        {
            self.profile.capabilities.build = CapabilityStatus::Unknown;
            self.profile.capabilities.check = CapabilityStatus::Unknown;
            self.profile.capabilities.test = CapabilityStatus::Unknown;
        }
        for command in &self.profile.commands {
            match command.kind {
                VerificationKind::Build => self.profile.capabilities.build = CommandsSuggested,
                VerificationKind::Test => self.profile.capabilities.test = CommandsSuggested,
                _ => self.profile.capabilities.check = CommandsSuggested,
            }
        }
        if !self.profile.commands.is_empty() {
            self.profile.support = SupportLevel::Discovered;
        }
        self.profile.languages.sort();
        self.profile.languages.dedup();
        self.profile
    }
}
