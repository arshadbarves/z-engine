use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use cap_std::fs::Dir;

use crate::{Diagnostic, DiscoveryError, DiscoveryOptions, ScanStats};

/// Every nested open is relative to this capability, including ignore files.
pub(crate) struct Workspace {
    pub root: PathBuf,
    pub directory: Dir,
}

impl Workspace {
    pub fn open(path: &Path) -> Result<Self, DiscoveryError> {
        let fail = |source| DiscoveryError::Workspace {
            path: path.into(),
            source,
        };
        let root = path.canonicalize().map_err(fail)?;
        if !root.is_dir() {
            return Err(DiscoveryError::NotDirectory(root));
        }
        let directory = Dir::open_ambient_dir(&root, cap_std::ambient_authority()).map_err(fail)?;
        Ok(Self { root, directory })
    }

    pub fn read_text(
        &self,
        path: &Path,
        options: &DiscoveryOptions,
        stats: &mut ScanStats,
    ) -> Result<String, Diagnostic> {
        let absolute = self.root.join(path);
        let fail = |error: std::io::Error| {
            Diagnostic::error(
                "read_failed",
                &absolute,
                format!("Cannot read manifest: {error}"),
            )
        };
        let metadata = self.directory.symlink_metadata(path).map_err(fail)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Diagnostic::error(
                "unsafe_file",
                absolute,
                "Manifest must be a regular, non-symlink file; rescan after fixing it.",
            ));
        }
        let remaining = options.max_total_bytes.saturating_sub(stats.bytes_read);
        let cap = remaining.min(options.max_manifest_bytes);
        if metadata.len() > cap as u64 {
            stats.complete = false;
            return Err(Diagnostic::warning(
                "byte_limit",
                absolute,
                format!(
                    "File is {} bytes; remaining per-file/total budget is {cap}. Increase discovery limits or inspect it separately.",
                    metadata.len()
                ),
            ));
        }
        let file = self.directory.open(path).map_err(fail)?;
        // Recheck the opened handle too: a concurrently replaced file is not trusted.
        if !file.metadata().map_err(fail)?.is_file() {
            return Err(Diagnostic::error(
                "unsafe_file",
                absolute,
                "Manifest changed into a non-regular file during discovery.",
            ));
        }
        let mut bytes = Vec::new();
        let mut reader = file.take(cap as u64);
        let read = reader.read_to_end(&mut bytes);
        stats.bytes_read += bytes.len();
        read.map_err(fail)?;
        if reader.get_ref().metadata().map_err(fail)?.len() > bytes.len() as u64 {
            stats.complete = false;
            return Err(Diagnostic::warning(
                "byte_limit",
                absolute,
                "Manifest grew beyond the read budget; increase limits and rescan.",
            ));
        }
        String::from_utf8(bytes).map_err(|error| {
            Diagnostic::error(
                "invalid_encoding",
                absolute,
                format!("Expected UTF-8 manifest text: {error}"),
            )
        })
    }
}

pub(crate) fn check_cancel(cancel: &AtomicBool) -> Result<(), DiscoveryError> {
    if cancel.load(Ordering::Relaxed) {
        Err(DiscoveryError::Cancelled)
    } else {
        Ok(())
    }
}
