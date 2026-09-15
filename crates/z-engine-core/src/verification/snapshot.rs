use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use super::VerificationError;
use super::artifacts::digest_file;

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SCAN_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub fingerprint: String,
    pub(super) root: PathBuf,
    pub(super) files: BTreeMap<String, String>,
}

impl WorkspaceSnapshot {
    /// Hash actual inputs, including dirty/untracked content, not just Git HEAD.
    pub fn capture(root: &Path) -> Result<Self, VerificationError> {
        let root = root
            .canonicalize()
            .map_err(|e| VerificationError::io(root, e))?;
        if !root.join("Cargo.toml").is_file() {
            return Err(VerificationError::Unsupported(
                "S1 verification requires a Rust Cargo workspace".into(),
            ));
        }
        let mut paths = git_paths(&root)?.unwrap_or_default();
        // Include ignored Rust/config inputs too: ignore rules are not build boundaries.
        let walker = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .follow_links(false)
            .filter_entry({
                let root = root.clone();
                move |entry| !entry.path().strip_prefix(&root).is_ok_and(excluded)
            })
            .build();
        let mut scanned = 0;
        for entry in walker {
            let entry = entry.map_err(|e| VerificationError::Unsupported(e.to_string()))?;
            scanned += 1;
            if scanned > MAX_FILES * 2 {
                return Err(VerificationError::ScanLimit(
                    "too many directory entries".into(),
                ));
            }
            let path = entry.path();
            if path == root {
                continue;
            }
            let relative = path.strip_prefix(&root).map_err(|e| {
                VerificationError::Unsupported(format!("workspace containment: {e}"))
            })?;
            if entry.file_type().is_some_and(|kind| kind.is_symlink()) {
                return Err(VerificationError::Unsupported(format!(
                    "symlinked workspace input: {}",
                    relative.display()
                )));
            }
            if entry.file_type().is_some_and(|kind| kind.is_file()) {
                paths.insert(relative.to_path_buf());
            }
        }
        if paths.len() > MAX_FILES {
            return Err(VerificationError::ScanLimit("too many source files".into()));
        }
        let mut files = BTreeMap::new();
        let mut total = 0_u64;
        for relative in paths {
            if excluded(&relative) {
                continue;
            }
            let path = root.join(&relative);
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(VerificationError::io(path, error)),
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(VerificationError::Unsupported(format!(
                    "external/submodule/non-regular input: {}",
                    path.display()
                )));
            }
            if path
                .canonicalize()
                .map_err(|e| VerificationError::io(&path, e))?
                != path
            {
                return Err(VerificationError::Unsupported(format!(
                    "symlinked input ancestor: {}",
                    path.display()
                )));
            }
            if relative
                .file_name()
                .is_some_and(|name| name == "Cargo.toml")
            {
                super::cargo_scope::validate_manifest(&root, &path)?;
            }
            if matches!(
                relative.to_str(),
                Some(".cargo/config" | ".cargo/config.toml")
            ) {
                super::cargo_scope::validate_config(&root, &path)?;
            }
            let (digest, size) = digest_file(&path, MAX_FILE_BYTES)?;
            total += size;
            if total > MAX_SCAN_BYTES {
                return Err(VerificationError::ScanLimit(
                    "source bytes exceed 256 MiB".into(),
                ));
            }
            let name = relative.to_str().ok_or_else(|| {
                VerificationError::Unsupported("non-UTF8 workspace input path".into())
            })?;
            files.insert(name.replace('\\', "/"), digest);
        }
        let mut hash = Sha256::new();
        hash.update(b"z-engine-workspace-v1\0");
        hash.update(root.as_os_str().as_encoded_bytes());
        for (path, digest) in &files {
            hash.update((path.len() as u64).to_le_bytes());
            hash.update(path.as_bytes());
            hash.update(digest.as_bytes());
        }
        Ok(Self {
            fingerprint: format!("{:x}", hash.finalize()),
            root,
            files,
        })
    }

    pub fn changed_paths(&self, current: &Self) -> Vec<String> {
        self.files
            .keys()
            .chain(current.files.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|path| self.files.get(*path) != current.files.get(*path))
            .cloned()
            .collect()
    }
}

pub(super) fn excluded(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | "target" | "node_modules" | ".z-engine" | ".zengine")
        )
    })
}

fn git_paths(root: &Path) -> Result<Option<BTreeSet<PathBuf>>, VerificationError> {
    let mut child = match Command::new("git")
        .env_clear()
        .envs(super::environment::effective())
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            ".",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(VerificationError::io(root, error)),
    };
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or_else(|| VerificationError::Process("Git manifest stdout unavailable".into()))?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes);
    if bytes.len() > 8 * 1024 * 1024 || read.is_err() {
        if let Err(error) = child.kill() {
            tracing::warn!(%error, "could not stop Git manifest process");
        }
        child.wait().map_err(|e| VerificationError::io(root, e))?;
        read.map_err(|e| VerificationError::io(root, e))?;
        return Err(VerificationError::ScanLimit(
            "Git manifest exceeds 8 MiB".into(),
        ));
    }
    if !child
        .wait()
        .map_err(|e| VerificationError::io(root, e))?
        .success()
    {
        return Ok(None);
    }
    bytes
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
        .map(|bytes| {
            let name = std::str::from_utf8(bytes)
                .map_err(|_| VerificationError::Unsupported("non-UTF8 Git input path".into()))?;
            let path = PathBuf::from(name);
            if path.is_absolute()
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(VerificationError::Unsupported(
                    "Git input outside workspace".into(),
                ));
            }
            Ok(path)
        })
        .collect::<Result<_, _>>()
        .map(Some)
}
