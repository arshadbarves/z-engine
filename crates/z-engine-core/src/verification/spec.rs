use super::{CheckKind, CheckSpec, VerificationError};

impl CheckSpec {
    pub fn validate(&self) -> Result<(), VerificationError> {
        if let Some(package) = &self.package {
            if package.is_empty()
                || package.len() > 128
                || !package.starts_with(|c: char| c.is_ascii_alphanumeric())
                || !package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            {
                return Err(VerificationError::InvalidInput(
                    "package must be a Cargo package name, not arguments".into(),
                ));
            }
        }
        if let Some(filter) = &self.filter {
            if self.kind != CheckKind::CargoTest
                || filter.is_empty()
                || filter.len() > 256
                || !filter.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                || !filter
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':'))
            {
                return Err(VerificationError::InvalidInput(
                    "filter must be a Rust test name; cargo_build disallows filters".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn command(&self) -> Result<Vec<String>, VerificationError> {
        self.validate()?;
        let mut args = vec![
            "cargo".into(),
            match self.kind {
                CheckKind::CargoTest => "test",
                CheckKind::CargoBuild => "build",
            }
            .into(),
        ];
        if let Some(package) = &self.package {
            args.extend(["--package".into(), package.clone()]);
        } else {
            args.push("--workspace".into());
        }
        if self.kind == CheckKind::CargoTest {
            // Selecting all targets suppresses Cargo's default doctest execution.
            args.push("--no-fail-fast".into());
        } else {
            args.push("--all-targets".into());
        }
        if let Some(filter) = &self.filter {
            args.push(filter.clone());
        }
        Ok(args)
    }

    pub fn is_full_workspace_test(&self) -> bool {
        self.kind == CheckKind::CargoTest && self.package.is_none() && self.filter.is_none()
    }
}
