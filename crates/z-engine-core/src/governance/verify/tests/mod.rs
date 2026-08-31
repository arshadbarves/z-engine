//! Verification-runner tests: shared fixtures, plus the two concerns
//! they serve.
//!
//! Split out of `verify.rs` so the runner file stays about the rules,
//! and split again by concern: `checks` covers what running a check
//! proves, `scope` covers what the declared order accounts for.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::governance::manifest::Verdict;

const LIB: &str = "pub fn parse(s: &str) -> usize {\n    s.len()\n}\n";
const MANIFEST: &str = "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";

/// A dependency-free cargo project, so `cargo check` is a real verdict
/// that needs no network and compiles in well under a second.
fn cargo_fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), LIB).unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), MANIFEST).unwrap();
    tmp
}

fn accept(command: &str) -> Vec<AcceptanceCommand> {
    vec![AcceptanceCommand {
        command: command.into(),
        description: "acceptance".into(),
    }]
}

fn plan(scope: &[&str], mutated: &[&str], acceptance: Vec<AcceptanceCommand>) -> VerificationPlan {
    VerificationPlan {
        work_order_id: "wo-1".into(),
        goal: "make parse fallible".into(),
        scope: scope.iter().map(PathBuf::from).collect(),
        mutated: mutated.iter().map(PathBuf::from).collect(),
        witnesses: Vec::new(),
        acceptance,
    }
}

fn witness(root: &std::path::Path, rel: &str) -> ReadWitness {
    let bytes = std::fs::read(root.join(rel)).unwrap();
    ReadWitness {
        path: PathBuf::from(rel),
        file_hash: BlobHandle::of(&bytes).to_string(),
    }
}

fn check<'a>(m: &'a VerificationManifest, name: &str) -> &'a CheckOutcome {
    m.checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no {name} check in {:?}", m.checks))
}

mod checks;
mod scope;
