//! Per-turn file checkpoints powering rewind (Claude Code / Codex style).
//!
//! Before `edit_file` / `write_file` first touches a file within a turn,
//! the on-disk pre-image is stashed here. Rewinding pops the most recent
//! non-empty turn and restores those files (created files are removed).
//! Files mutated through `bash` are not tracked — documented limitation.
//!
//! The write-back half lives in [`checkpoint_restore`]; `RevertOutcome` is
//! re-exported here to keep the public path stable.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::checkpoint_restore::restore_files;

pub use super::checkpoint_restore::RevertOutcome;

/// Largest pre-image kept per file; bigger files are skipped entirely
/// (they stay untracked rather than being half-revertible).
const MAX_SNAPSHOT_BYTES: u64 = 8 * 1024 * 1024;
/// Cap on retained turns so marathon sessions don't grow without bound.
const MAX_TURNS: usize = 50;

#[derive(Debug)]
pub(super) struct TouchedFile {
    pub(super) path: PathBuf,
    /// None = the file did not exist before the turn (agent created it).
    pub(super) original: Option<Vec<u8>>,
}

/// One turn's snapshot batch. `id` is globally monotonic across the
/// session so external callers (UI message indexes) can address turns
/// even after eviction has shifted the internal Vec positions.
#[derive(Debug)]
struct Turn {
    id: u64,
    files: Vec<TouchedFile>,
}

#[derive(Debug, Default)]
pub struct CheckpointStore {
    turns: Mutex<Vec<Turn>>,
}

impl CheckpointStore {
    fn next_id(turns: &[Turn]) -> u64 {
        turns.last().map(|t| t.id + 1).unwrap_or(0)
    }

    /// Open a fresh (empty) checkpoint for the turn about to run.
    pub fn begin_turn(&self) {
        if let Ok(mut turns) = self.turns.lock() {
            let id = Self::next_id(&turns);
            if turns.len() >= MAX_TURNS {
                turns.remove(0);
            }
            turns.push(Turn {
                id,
                files: Vec::new(),
            });
        }
    }

    /// Stash the current on-disk state of `path` for this turn, once.
    pub fn snapshot_file(&self, path: &Path) {
        let Ok(mut turns) = self.turns.lock() else {
            return;
        };
        let Some(current) = turns.last_mut() else {
            return; // no turn open (e.g. tool called outside the loop)
        };
        if current.files.iter().any(|t| t.path == path) {
            return; // already hold the oldest (correct) pre-image
        }
        let original: Option<Vec<u8>> = match std::fs::metadata(path) {
            // Missing ⇒ agent-created; revert removes it.
            Err(_) => None,
            Ok(meta) => {
                // Untrackable (too large / not a regular file): skip the
                // file entirely rather than risk deleting it on rewind.
                if !meta.is_file() || meta.len() > MAX_SNAPSHOT_BYTES {
                    return;
                }
                match std::fs::read(path) {
                    Ok(bytes) => Some(bytes),
                    Err(_) => return,
                }
            }
        };
        current.files.push(TouchedFile {
            path: path.to_path_buf(),
            original,
        });
    }

    /// Undo every checkpointed turn with id >= `keep`, leaving turns with
    /// smaller ids intact. This powers per-message revert: reverting user
    /// message N passes `keep = N`. Turn ids are monotonic and never
    /// reused, so the mapping stays correct even after old turns were
    /// evicted to honor MAX_TURNS. Restoring walks newest-first so older
    /// pre-images overwrite newer edits of the same file. If some turns
    /// in `[keep..]` were already evicted, everything still retained is
    /// reverted and [`RevertOutcome::evicted_gaps`] is set — those files'
    /// pre-images are gone.
    pub fn revert_to_turn(&self, keep: u64) -> RevertOutcome {
        let mut out = RevertOutcome::default();
        let targets: Vec<Vec<TouchedFile>> = {
            let Ok(mut turns) = self.turns.lock() else {
                return out;
            };
            let Some(first) = turns.iter().position(|t| t.id >= keep) else {
                return out; // nothing at or after `keep` is retained
            };
            // The retained window is always a suffix of created turns,
            // so a higher-than-requested first id means older turns in
            // [keep..first) were evicted and their pre-images are gone.
            out.evicted_gaps = turns[first].id > keep;
            turns.drain(first..).map(|t| t.files).collect()
        };
        for files in targets.iter().rev() {
            restore_files(files, &mut out);
        }
        out
    }

    /// Undo the most recent checkpointed turn. Empty checkpoints between
    /// turns (no mutating tools ran) are skipped/discarded.
    pub fn revert_last_turn(&self) -> RevertOutcome {
        let mut out = RevertOutcome::default();
        let target = {
            let Ok(mut turns) = self.turns.lock() else {
                return out;
            };
            while turns.last().is_some_and(|t| t.files.is_empty()) {
                turns.pop();
            }
            turns.pop().map(|t| t.files)
        };
        let Some(files) = target else {
            return out; // nothing to revert
        };
        // Order within one turn is irrelevant:
        // every entry holds that file's unique pre-turn image.
        restore_files(&files, &mut out);
        out
    }

    /// Number of pending (non-empty) checkpoints available to rewind.
    pub fn pending_turns(&self) -> usize {
        self.turns
            .lock()
            .map(|t| t.iter().filter(|turn| !turn.files.is_empty()).count())
            .unwrap_or(0)
    }

    /// Earliest retained pre-image per absolute path (session baseline).
    /// First touch wins so later edits of the same file keep the pre-session
    /// (or pre-first-touch) content for chat-scoped diffs.
    pub fn earliest_baselines(&self) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        let Ok(turns) = self.turns.lock() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for turn in turns.iter() {
            for f in &turn.files {
                if seen.insert(f.path.clone()) {
                    out.push((f.path.clone(), f.original.clone()));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> CheckpointStore {
        CheckpointStore::default()
    }

    #[test]
    fn first_snapshot_wins_across_repeated_edits() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("g.txt");
        std::fs::write(&p, "original").unwrap();

        let store = store();
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "edit 1").unwrap();
        store.snapshot_file(&p); // second edit same turn: ignored
        std::fs::write(&p, "edit 2").unwrap();

        store.revert_last_turn();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "original");
    }

    #[test]
    fn empty_turns_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("h.txt");
        std::fs::write(&p, "a").unwrap();

        let store = store();
        store.begin_turn(); // turn 1 edits
        store.snapshot_file(&p);
        std::fs::write(&p, "b").unwrap();

        store.revert_last_turn();

        store.begin_turn(); // read-only turn: nothing snapshotted

        // Rewinding with only empty turns left is a clean no-op.
        let out = store.revert_last_turn();
        assert!(out.restored.is_empty());
    }

    #[test]
    fn multi_step_rewind_walks_backwards() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("k.txt");
        std::fs::write(&p, "s0").unwrap();

        let store = store();
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "s1").unwrap();

        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "s2").unwrap();

        store.revert_last_turn();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "s1");
        store.revert_last_turn();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "s0");
    }

    #[test]
    fn revert_to_turn_restores_everything_after_the_kept_point() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, "a0").unwrap();

        let store = store();
        // turn 0 edits a
        store.begin_turn();
        store.snapshot_file(&a);
        std::fs::write(&a, "a1").unwrap();
        // turn 1 creates b
        store.begin_turn();
        store.snapshot_file(&b); // did not exist
        std::fs::write(&b, "b1").unwrap();
        // turn 2 edits a again
        store.begin_turn();
        store.snapshot_file(&a);
        std::fs::write(&a, "a2").unwrap();

        // Revert to before turn 1 (keep only turn 0): b must vanish and
        // a's second edit must roll back to a1.
        let out = store.revert_to_turn(1);
        assert_eq!(out.restored.len(), 2, "{:?}", out.restored);
        assert!(out.errors.is_empty());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "a1");
        assert!(!b.exists());
        assert_eq!(store.pending_turns(), 1);
    }

    #[test]
    fn revert_to_turn_older_preimages_win_over_newer() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("x.txt");
        std::fs::write(&p, "v0").unwrap();

        let store = store();
        for v in ["v1", "v2", "v3"] {
            store.begin_turn();
            store.snapshot_file(&p);
            std::fs::write(&p, v).unwrap();
        }
        store.revert_to_turn(0);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v0");
    }

    #[test]
    fn revert_to_turn_beyond_stack_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("y.txt");
        std::fs::write(&p, "orig").unwrap();

        let store = store();
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "edited").unwrap();

        let out = store.revert_to_turn(5);
        assert!(out.restored.is_empty());
        assert!(!out.evicted_gaps);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "edited");
    }

    #[test]
    fn revert_ids_stay_stable_after_eviction() {
        // Regression: MAX_TURNS eviction used to shift Vec indices so a
        // UI-supplied turn number addressed the WRONG slice. Monotonic
        // ids keep the mapping correct after eviction.
        let tmp = tempfile::tempdir().unwrap();
        let early = tmp.path().join("early.txt");
        let late = tmp.path().join("late.txt");
        std::fs::write(&early, "e0").unwrap();
        std::fs::write(&late, "l0").unwrap();

        let store = store();
        // Turn 0 edits `early`; turns 1..=MAX_TURNS+1 edit `late`, pushing
        // turn 0 out of the retained window.
        store.begin_turn();
        store.snapshot_file(&early);
        std::fs::write(&early, "e1").unwrap();
        for i in 1..=(MAX_TURNS + 1) {
            store.begin_turn();
            store.snapshot_file(&late);
            let _ = std::fs::write(&late, format!("l{i}"));
        }
        assert_eq!(store.pending_turns(), MAX_TURNS);

        // Revert everything from turn 1 on. Turn 0's slot was evicted,
        // and so was turn 1's own record (the retained window is a
        // suffix), so the rewind is best-effort down to the oldest
        // retained pre-image ("l1") — but crucially it addresses exactly
        // the right window instead of an off-by-N slice.
        let out = store.revert_to_turn(1);
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert!(out.evicted_gaps);
        assert_eq!(std::fs::read_to_string(&late).unwrap(), "l1");
        // `early` was only ever touched by turn 0 (before `keep`), so its
        // edit must survive untouched.
        assert_eq!(std::fs::read_to_string(&early).unwrap(), "e1");
    }

    #[test]
    fn revert_reports_evicted_gaps() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("g.txt");
        std::fs::write(&p, "v0").unwrap();

        let store = store();
        // Turn 0 edits p; MAX_TURNS further turns push turn 0's slot out.
        store.begin_turn();
        store.snapshot_file(&p);
        std::fs::write(&p, "v1").unwrap();
        for i in 2..=(MAX_TURNS + 1) {
            store.begin_turn();
            store.snapshot_file(&p);
            let _ = std::fs::write(&p, format!("v{i}"));
        }
        // Revert to turn 0: its pre-image was evicted, so the best-effort
        // rewind restores only the oldest retained pre-image ("v1") and
        // must report the gap instead of silently claiming success.
        let out = store.revert_to_turn(0);
        assert!(out.evicted_gaps);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v1");
    }

    #[test]
    fn oversized_files_are_not_tracked() {
        // A file larger than MAX_SNAPSHOT_BYTES must not blow up memory;
        // it simply stays out of the checkpoint (revert skips it).
        let tmp = tempfile::tempdir().unwrap();
        let big = tmp.path().join("big.bin");
        std::fs::write(&big, vec![0u8; (MAX_SNAPSHOT_BYTES + 1) as usize]).unwrap();

        let store = store();
        store.begin_turn();
        store.snapshot_file(&big);

        // Nothing recorded → nothing to revert.
        let out = store.revert_last_turn();
        assert!(out.restored.is_empty());
        assert!(big.exists());
    }
}
