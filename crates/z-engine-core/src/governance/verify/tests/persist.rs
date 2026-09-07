//! Manifest-persistence tests: a run's account of itself is append-only,
//! one record per turn, and never replaced.

use super::*;
use std::path::PathBuf;

fn manifest(goal: &str) -> VerificationManifest {
    VerificationManifest {
        work_order_id: "wo-1".into(),
        goal: goal.into(),
        scope: vec![PathBuf::from("src/lib.rs")],
        mutated: vec![],
        breaches: vec![],
        checks: vec![],
    }
}

fn entries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_manifest_is_written_where_the_refusal_can_point_at_it() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runs/01ABC");
    let manifest = manifest("g");
    let path = write_manifest(&dir, 1, &manifest).unwrap();
    assert_eq!(path, dir.join("verification-1.json"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        serde_json::from_str::<VerificationManifest>(&text).unwrap(),
        manifest
    );
    // The old, turn-less name survives as a pointer at the newest record.
    assert_eq!(
        std::fs::read_to_string(dir.join("verification.json")).unwrap(),
        text
    );
}

/// A run is not one turn. Turn 2's verdict must not erase the record that
/// turn 1 was verified — a refusal that overwrote the proof of the work
/// it followed would leave the run unable to account for itself.
#[test]
fn every_turn_keeps_its_own_manifest_and_the_pointer_follows_the_newest() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runs/01ABC");

    let first = manifest("turn one: verified");
    write_manifest(&dir, 1, &first).unwrap();
    let second = manifest("turn two: a different verdict entirely");
    let second_path = write_manifest(&dir, 2, &second).unwrap();

    assert_eq!(
        entries(&dir),
        [
            "verification-1.json",
            "verification-2.json",
            "verification.json"
        ],
        "each turn keeps its own record, beside one pointer"
    );
    assert_eq!(
        serde_json::from_str::<VerificationManifest>(
            &std::fs::read_to_string(dir.join("verification-1.json")).unwrap()
        )
        .unwrap(),
        first,
        "turn 1's manifest must survive turn 2"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("verification.json")).unwrap(),
        std::fs::read_to_string(&second_path).unwrap(),
        "the pointer names the newest turn"
    );
    // Each record says which turn it is, so the pointer is never the only
    // way to tell them apart.
    let turns: Vec<u64> = ["verification-1.json", "verification-2.json"]
        .iter()
        .map(|name| {
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(dir.join(name)).unwrap(),
            )
            .unwrap()["turn"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(turns, [1, 2]);
}

/// Filing two manifests under one turn number would destroy evidence, so
/// it is refused rather than silently honoured.
#[test]
fn a_repeated_turn_number_is_refused_instead_of_replacing_the_record() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runs/01ABC");
    write_manifest(&dir, 1, &manifest("the real verdict")).unwrap();

    let err = write_manifest(&dir, 1, &manifest("a replacement")).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists, "{err}");
    assert_eq!(
        serde_json::from_str::<VerificationManifest>(
            &std::fs::read_to_string(dir.join("verification-1.json")).unwrap()
        )
        .unwrap(),
        manifest("the real verdict")
    );
}

/// The manifest is the artefact a refusal points at, so it is written the
/// way every other durable artefact in this crate is: whole or not at all,
/// leaving no half-written file and no temporary debris behind.
#[test]
fn the_manifest_is_written_atomically_and_leaves_nothing_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runs/01ABC");
    write_manifest(&dir, 1, &manifest("g")).unwrap();
    let longer = manifest("a second, longer verdict for the same run");
    let path = write_manifest(&dir, 2, &longer).unwrap();

    assert_eq!(
        entries(&dir),
        [
            "verification-1.json",
            "verification-2.json",
            "verification.json"
        ],
        "an atomic write leaves no temporary file behind"
    );
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        serde_json::from_str::<VerificationManifest>(&text).unwrap(),
        longer,
        "the rewrite must replace the whole file, not overlay it"
    );
}
