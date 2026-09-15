//! Filesystem discovery only: no commands, network requests, or completion evidence.

mod discovery;
mod error;
mod filesystem;
mod markers;
mod options;
mod parsers;
mod traversal;
mod types;

pub use discovery::{discover, discover_with_cancel};
pub use error::DiscoveryError;
pub use options::DiscoveryOptions;
pub use types::{
    CapabilityStatus, CommandBasis, CommandSuggestion, Diagnostic, DiagnosticSeverity,
    ExecutionStatus, ProfileCapabilities, ProjectKind, ProjectProfile, ProjectReport, ScanStats,
    SupportLevel, VerificationKind,
};
