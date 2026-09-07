//! Where a run's evidence ids come from.
//!
//! An evidence id is the one genuinely arbitrary value in a guarded run,
//! and it reaches the model inside every `read_file` result — so it is
//! also the one value a replay has to be handed rather than invent.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use ulid::Ulid;

use super::error::ReplayError;
use super::tape::RunCassette;

/// What identifies a read for the purpose of reclaiming its id: the path
/// and the range, never the order it happened to arrive in. One round
/// can read two windows of the same file concurrently, and which of
/// those finishes first is the scheduler's business, not the run's.
type ReadKey = (String, Option<(u32, u32)>);

/// Ids the recorded run minted, waiting to be claimed by the same reads.
#[derive(Default)]
pub(super) struct ReplayIds {
    queues: HashMap<ReadKey, VecDeque<String>>,
    served: HashMap<ReadKey, usize>,
}

/// Fresh ids for a live run; recorded ids for a replayed one.
pub(super) enum IdSource {
    /// Mint fresh ULIDs, exactly as an unrecorded run does.
    Fresh,
    /// Hand back the ids the recorded run minted, keyed by the read they
    /// were minted for.
    Replayed(Mutex<ReplayIds>),
}

impl IdSource {
    /// Serve back the ids `source` minted.
    pub(super) fn from_cassette(source: &RunCassette) -> Self {
        let mut queues: HashMap<ReadKey, VecDeque<String>> = HashMap::new();
        for mint in source.evidence_ids() {
            queues
                .entry((mint.path, mint.range))
                .or_default()
                .push_back(mint.id);
        }
        IdSource::Replayed(Mutex::new(ReplayIds {
            queues,
            served: HashMap::new(),
        }))
    }

    pub(super) fn is_replaying(&self) -> bool {
        matches!(self, IdSource::Replayed(_))
    }

    /// The id for a read of `path` over `range`.
    ///
    /// Replaying a read the recorded run never made is a divergence, not
    /// a detail to paper over, so this fails rather than minting: a run
    /// that invents an id here would diverge on its very next request
    /// anyway, but only after writing a fiction into the ledger.
    pub(super) fn claim(
        &self,
        path: &str,
        range: Option<(u32, u32)>,
    ) -> Result<String, ReplayError> {
        let ids = match self {
            IdSource::Fresh => return Ok(Ulid::new().to_string()),
            IdSource::Replayed(ids) => ids,
        };
        let key: ReadKey = (path.to_string(), range);
        let mut ids = ids.lock().expect("recorder ids");
        let ordinal = ids.served.get(&key).copied().unwrap_or(0);
        match ids.queues.get_mut(&key).and_then(VecDeque::pop_front) {
            Some(id) => {
                ids.served.insert(key, ordinal + 1);
                Ok(id)
            }
            None => Err(ReplayError::EvidenceExhausted {
                path: path.to_string(),
                ordinal,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::RunRecorder;
    use crate::replay::entry::EvidenceMint;

    /// A cassette that recorded a whole-file read of each path.
    fn recorded(paths: &[&str], path: &std::path::Path) -> RunCassette {
        let recorder = RunRecorder::recording(path).unwrap();
        for p in paths {
            recorder.evidence_id(p, None).unwrap();
        }
        RunCassette::load(path).unwrap()
    }

    #[test]
    fn a_live_run_mints_a_fresh_id_every_time() {
        let source = IdSource::Fresh;
        let first = source.claim("src/lib.rs", None).unwrap();
        let second = source.claim("src/lib.rs", None).unwrap();
        assert_ne!(first, second);
        assert!(!source.is_replaying());
    }

    #[test]
    fn a_read_claims_the_id_minted_for_that_same_path_and_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let recorder = RunRecorder::recording(&path).unwrap();
        let whole = recorder.evidence_id("src/lib.rs", None).unwrap();
        let window = recorder.evidence_id("src/lib.rs", Some((40, 60))).unwrap();
        let other = recorder.evidence_id("Cargo.toml", None).unwrap();

        // Replay claims them in a different order than they were minted:
        // concurrent reads of one file must not swap ids.
        let source = IdSource::from_cassette(&RunCassette::load(&path).unwrap());
        assert!(source.is_replaying());
        assert_eq!(source.claim("src/lib.rs", Some((40, 60))).unwrap(), window);
        assert_eq!(source.claim("Cargo.toml", None).unwrap(), other);
        assert_eq!(source.claim("src/lib.rs", None).unwrap(), whole);
    }

    #[test]
    fn repeated_reads_of_one_window_are_served_in_the_order_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let cassette = recorded(&["a.rs", "a.rs", "b.rs"], &path);
        let minted: Vec<EvidenceMint> = cassette.evidence_ids();
        let source = IdSource::from_cassette(&cassette);
        assert_eq!(source.claim("a.rs", None).unwrap(), minted[0].id);
        assert_eq!(source.claim("a.rs", None).unwrap(), minted[1].id);
    }

    #[test]
    fn a_read_the_recorded_run_never_made_fails_instead_of_minting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let cassette = recorded(&["src/lib.rs"], &path);
        let source = IdSource::from_cassette(&cassette);

        source.claim("src/lib.rs", None).unwrap();
        let exhausted = source.claim("src/lib.rs", None).unwrap_err();
        assert!(
            matches!(exhausted, ReplayError::EvidenceExhausted { ordinal: 1, .. }),
            "an extra read must fail with its position, not a fresh id: {exhausted}"
        );
        let unseen = source.claim("unseen.rs", None).unwrap_err();
        assert!(matches!(
            unseen,
            ReplayError::EvidenceExhausted { ordinal: 0, .. }
        ));
    }
}
