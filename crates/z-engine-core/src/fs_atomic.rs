//! One crash-safe file replacement, shared by every subsystem that must
//! not leave a half-written artifact behind.
//!
//! Kept at the crate root rather than inside `evidence`, `governance`, or
//! `tools` because all three need it and none of them may depend on the
//! others: evidence storage must not couple to tools, and governance must
//! not couple to either. It stays synchronous and dependency-free so a
//! caller in any layer — including one holding a lock or running outside
//! an async context — can use it unchanged.
//!
//! `tools::fsutil::atomic_write` is the async twin for tool edits, which
//! already run inside the runtime and write model-sized payloads.

use std::io::Write;
use std::path::Path;

/// Write `bytes` to `target` atomically and durably: a temp sibling is
/// written, flushed to disk, then renamed over the target. A crash
/// mid-write can never leave the target truncated or half-written (POSIX
/// rename is atomic; on Windows a same-volume rename is best-effort but
/// still far safer than in-place truncation).
///
/// The parent directory is created only if the temp file cannot be
/// created because it is missing, so the common case pays no extra
/// syscall while a directory removed out-of-band still succeeds.
pub(crate) fn atomic_write(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| std::io::Error::other("target has no parent directory"))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        ulid::Ulid::new()
    ));
    let mut file = match std::fs::File::create(&tmp) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(dir)?;
            std::fs::File::create(&tmp)?
        }
        Err(e) => return Err(e),
    };
    let written = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    match std::fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_a_file_leaves_no_temporary_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("verification.json");
        std::fs::write(&target, b"old").unwrap();

        atomic_write(&target, b"new").unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n != "verification.json")
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn a_missing_parent_directory_is_created_once() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("runs/01ABC/verification.json");
        atomic_write(&target, b"{}").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"{}");
    }

    /// A target that cannot be written is an error the caller reports,
    /// never a silent no-op that would leave a refusal pointing at
    /// nothing.
    #[test]
    fn an_unwritable_target_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        assert!(atomic_write(&blocker.join("child.json"), b"{}").is_err());
    }
}
