use std::io::Read;
use std::path::Path;

use super::VerificationError;

pub(super) fn validate_manifest(root: &Path, path: &Path) -> Result<(), VerificationError> {
    let manifest = read_toml(path)?;
    let base = path.parent().ok_or_else(|| {
        VerificationError::InvalidInput("Cargo manifest has no containing directory".into())
    })?;
    if base == root && manifest.get("workspace").is_none() {
        for ancestor in root.ancestors().skip(1) {
            let parent_manifest = ancestor.join("Cargo.toml");
            if parent_manifest.exists() {
                return Err(VerificationError::Unsupported(format!(
                    "workspace membership may extend beyond the selected root: {}",
                    parent_manifest.display()
                )));
            }
        }
    }
    validate_values(root, base, &manifest)?;
    if let Some(workspace) = manifest.get("workspace") {
        for key in ["members", "default-members", "exclude"] {
            if let Some(values) = workspace.get(key).and_then(toml::Value::as_array) {
                for value in values {
                    if let Some(value) = value.as_str() {
                        validate_relative(root, base, value)?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_config(root: &Path, path: &Path) -> Result<(), VerificationError> {
    let config = read_toml(path)?;
    // Cargo resolves paths relative to the directory containing `.cargo`.
    let base = path.parent().and_then(Path::parent).ok_or_else(|| {
        VerificationError::Unsupported("Cargo configuration has no resolution root".into())
    })?;
    validate_values(root, base, &config)?;
    if let Some(paths) = config.get("paths").and_then(toml::Value::as_array) {
        for path in paths {
            if let Some(path) = path.as_str() {
                validate_relative(root, base, path)?;
            }
        }
    }
    Ok(())
}

fn read_toml(path: &Path) -> Result<toml::Value, VerificationError> {
    const LIMIT: u64 = 1024 * 1024;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| VerificationError::io(path, e))?
        .take(LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|e| VerificationError::io(path, e))?;
    if text.len() as u64 > LIMIT {
        return Err(VerificationError::ScanLimit(format!(
            "Cargo configuration exceeds 1 MiB: {}",
            path.display()
        )));
    }
    toml::from_str(&text)
        .map_err(|e| VerificationError::Unsupported(format!("{}: {e}", path.display())))
}

fn validate_values(root: &Path, base: &Path, value: &toml::Value) -> Result<(), VerificationError> {
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                if key == "harness" && value.as_bool() == Some(false) {
                    return Err(VerificationError::Unsupported(
                        "custom Rust harnesses cannot provide standard test-count evidence".into(),
                    ));
                }
                if matches!(key.as_str(), "path" | "workspace" | "directory" | "build") {
                    if let Some(path) = value.as_str() {
                        validate_relative(root, base, path)?;
                    }
                }
                validate_values(root, base, value)?;
            }
        }
        toml::Value::Array(values) => {
            for value in values {
                validate_values(root, base, value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_relative(root: &Path, base: &Path, value: &str) -> Result<(), VerificationError> {
    let joined = base.join(value);
    let mut normalized = std::path::PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            component => normalized.push(component.as_os_str()),
        }
    }
    if !normalized.starts_with(root)
        || normalized
            .strip_prefix(root)
            .is_ok_and(super::snapshot::excluded)
    {
        return Err(VerificationError::Unsupported(format!(
            "Cargo source scope is outside fingerprinted inputs: {value}"
        )));
    }
    if joined.exists() {
        let canonical = joined
            .canonicalize()
            .map_err(|e| VerificationError::io(&joined, e))?;
        if canonical != normalized || !canonical.starts_with(root) {
            return Err(VerificationError::Unsupported(format!(
                "symlinked Cargo source scope: {value}"
            )));
        }
    }
    Ok(())
}
