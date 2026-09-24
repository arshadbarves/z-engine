//! Verification evidence for any ecosystem: bounded, read-only check
//! discovery; running one check through the host shell into a protocol
//! `CheckRecord`; test-count parsing; check selection for changed paths;
//! and the per-turn outcome badge. Verification is non-blocking: it records
//! evidence and never decides whether the user may finish.

mod artifact;
mod assess;
mod discovery;
mod error;
mod parse;
mod run;
mod select;
mod spec;

pub use assess::assess;
pub use discovery::{DiscoveryOptions, ProjectProfile, ProjectRoot, discover, merge_configured};
pub use error::VerifyError;
pub use parse::parse_counts;
pub use run::{CheckEnv, run_check};
pub use select::select_checks;
pub use spec::{CheckSource, CheckSpec, ConfiguredCheck, DEFAULT_CHECK_TIMEOUT_SECS};
