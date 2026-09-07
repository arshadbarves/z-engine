//! Verification-runner tests: shared fixtures, plus the two concerns
//! they serve.
//!
//! Split out of `verify.rs` so the runner file stays about the rules,
//! and split again by concern: `checks` covers what running a check
//! proves, `scope` covers what the declared order accounts for.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::evidence::BlobHandle;
use crate::governance::manifest::Verdict;
use crate::governance::plan::{MutationRecord, ReadWitness};

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

impl VerificationRunner {
    /// Only what the checks proved. Most tests here are about the
    /// verdict rather than about the state the checks left behind, and
    /// say so by asking for the manifest alone.
    async fn proved(&self, plan: &VerificationPlan) -> VerificationManifest {
        self.run(plan).await.manifest
    }
}

fn accept(command: &str) -> Vec<AcceptanceCommand> {
    vec![AcceptanceCommand {
        command: command.into(),
        description: "acceptance".into(),
    }]
}

/// A plan whose mutation log agrees with what is on disk, which is what
/// an honest run produces: the tools wrote those bytes and nothing has
/// touched them since.
fn plan(
    root: &std::path::Path,
    scope: &[&str],
    mutated: &[&str],
    acceptance: Vec<AcceptanceCommand>,
) -> VerificationPlan {
    VerificationPlan {
        work_order_id: "wo-1".into(),
        goal: "make parse fallible".into(),
        scope: scope.iter().map(PathBuf::from).collect(),
        mutated: mutated
            .iter()
            .map(|rel| MutationRecord {
                path: PathBuf::from(rel),
                content_hash: hash_on_disk(root, rel),
            })
            .collect(),
        changes: Vec::new(),
        // The tree as it stands before the checks run; a default here
        // would make every file in the fixture look like something a
        // check created.
        workspace: WorkspaceSnapshot::capture(root, None).unwrap(),
        witnesses: Vec::new(),
        acceptance,
    }
}

fn hash_on_disk(root: &std::path::Path, rel: &str) -> String {
    std::fs::read(root.join(rel))
        .map(|bytes| BlobHandle::of(&bytes).to_string())
        .unwrap_or_default()
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
