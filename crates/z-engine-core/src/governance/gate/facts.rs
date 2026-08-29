//! The facts the gate weighs. Data only: gathering them is the tools
//! layer's job, judging them is [`super::engine`]'s.

use std::path::Path;

use crate::governance::ActiveWorkOrder;

/// A 1-based, inclusive line span. `None` anywhere in this module means
/// "the whole file", never "unknown".
pub type LineRange = (u32, u32);

/// What the run knows about the read evidence for the bytes it is about
/// to overwrite. Produced from the *same* snapshot the mutating tool is
/// working on, so nothing can change underneath the decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceState {
    /// This run never read the file.
    Missing,
    /// A read exists, but the bytes about to change are not those bytes.
    Stale,
    /// The captured read still matches; `covered` is what it saw.
    Fresh {
        /// Record id, carried so refusals and audits can name the read.
        id: String,
        /// Lines the read captured; `None` for a whole-file read.
        covered: Option<LineRange>,
    },
}

/// Whether the Rust semantic provider can answer questions right now.
/// Governance defines this itself rather than importing an LSP type: the
/// gate must not know (or care) which provider answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticHealth {
    Ready,
    Unavailable { reason: String },
}

/// The Rust-specific facts, supplied only when the target *is* Rust
/// source. `None` in [`MutationRequest::rust`] means no semantic claim is
/// being made, so none is demanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFacts {
    pub health: SemanticHealth,
    /// Symbol names declared in the bytes about to change, as discovered
    /// by the repo map's tree-sitter outline.
    pub declared: Vec<String>,
}

/// Everything the gate weighs for one mutation, gathered by the caller.
#[derive(Debug)]
pub struct MutationRequest<'a> {
    /// The path as the model spelled it — used in messages only.
    pub path: &'a Path,
    /// Canonical repository-relative identity; `None` when the path
    /// resolves outside the project root.
    pub identity: Option<&'a str>,
    /// The order in force for this run, if one was declared.
    pub order: Option<&'a ActiveWorkOrder>,
    /// Lines this change touches, in the file's *current* coordinates.
    /// `None` means the whole file is replaced or created.
    pub changed: Option<LineRange>,
    pub evidence: EvidenceState,
    pub rust: Option<RustFacts>,
}
