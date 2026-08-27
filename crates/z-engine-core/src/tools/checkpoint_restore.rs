//! Restore half of the checkpoint subsystem: writing a turn's stashed
//! pre-images back to disk. `None` originals mean the agent created the
//! file, so removal restores "never existed".

use std::path::PathBuf;

use super::checkpoint::TouchedFile;

/// Result of a rewind: what was restored, what failed, and whether any
/// turns had already been evicted.
#[derive(Debug, Default)]
pub struct RevertOutcome {
    pub restored: Vec<PathBuf>,
    pub errors: Vec<String>,
    /// Some turns in `[keep..]` had been evicted before this revert, so
    /// their pre-images could not be restored (best-effort rewind).
    pub evicted_gaps: bool,
}

/// Write back one turn's pre-images.
pub(super) fn restore_files(files: &[TouchedFile], out: &mut RevertOutcome) {
    for f in files {
        match &f.original {
            Some(bytes) => {
                if let Some(parent) = f.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&f.path, bytes) {
                    out.errors.push(format!("{}: {e}", f.path.display()));
                } else {
                    out.restored.push(f.path.clone());
                }
            }
            None => match std::fs::remove_file(&f.path) {
                Ok(()) => out.restored.push(f.path.clone()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    out.restored.push(f.path.clone());
                }
                Err(e) => out.errors.push(format!("{}: {e}", f.path.display())),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::checkpoint::CheckpointStore;

    #[test]
    fn snapshot_then_revert_restores_content() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("f.txt");
        std::fs::write(&p, "v1").unwrap();

        let store = CheckpointStore::default();
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "v2 by agent").unwrap();

        let out = store.revert_last_turn();
        assert_eq!(out.restored, vec![p.clone()]);
        assert!(out.errors.is_empty());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v1");
        assert_eq!(store.pending_turns(), 0);
    }

    #[test]
    fn created_files_are_removed_on_revert() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("new.txt");

        let store = CheckpointStore::default();
        store.begin_turn();
        store.snapshot_file(&p); // did not exist
        std::fs::write(&p, "agent-made").unwrap();

        let out = store.revert_last_turn();
        assert_eq!(out.restored, vec![p.clone()]);
        assert!(!p.exists());
    }
}
