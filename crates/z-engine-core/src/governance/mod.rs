//! Governance boundary: typed work orders binding an agent's goal to
//! evidence-backed writable paths, a pure prompt builder that assembles
//! model instructions deterministically from a pinned snapshot, and the
//! fail-closed gate every guarded mutation must clear.
//!
//! Split by reason to change: the port onto workspace evidence
//! (`evidence_view`), admission rules (`work_order`), the admitted order
//! and its single-slot store (`active`), bounded prompt assembly
//! (`prompt`), and mutation authorization (`gate`).

pub mod active;
pub mod evidence_view;
pub mod gate;
pub mod prompt;
pub mod work_order;

pub use active::{ActiveWorkOrder, WorkOrderStore};
pub use evidence_view::EvidenceView;
pub use gate::{
    EvidenceState, GateDecision, GateEngine, GateFailure, LineRange, MutationRequest, RustFacts,
    SemanticHealth, changed_line_range,
};
pub use prompt::{PromptManifest, PromptOverflow, PromptSection, PromptSnapshot, build_prompt};
pub use work_order::{AcceptanceCommand, WorkOrder, WorkOrderError};
