//! Governance boundary: typed work orders binding an agent's goal to
//! evidence-backed writable paths, a pure prompt builder that assembles
//! model instructions deterministically from a pinned snapshot, the
//! fail-closed gate every guarded mutation must clear, and the
//! verification that must pass before a guarded run may call itself done.
//!
//! Split by reason to change: the port onto workspace evidence
//! (`evidence_view`), admission rules (`work_order`), the admitted order
//! and its single-slot store (`active`), bounded prompt assembly
//! (`prompt`), mutation authorization (`gate`), bounded subprocess
//! execution (`command_run`), completion checks (`verify`), and what
//! those checks proved (`manifest`).

pub mod active;
pub mod evidence_view;
pub mod gate;
pub mod manifest;
pub mod prompt;
pub mod verify;
pub mod work_order;

mod command_run;

pub use active::{ActiveWorkOrder, WorkOrderStore};
pub use evidence_view::EvidenceView;
pub use gate::{
    EvidenceState, GateDecision, GateEngine, GateFailure, LineRange, MutationRequest, RustFacts,
    SemanticEvidence, SemanticHealth, changed_line_range,
};
pub use manifest::{CheckOutcome, CheckStatus, ScopeBreach, Verdict, VerificationManifest};
pub use prompt::{PromptManifest, PromptOverflow, PromptSection, PromptSnapshot, build_prompt};
pub use verify::{ReadWitness, VerificationPlan, VerificationRunner, write_manifest};
pub use work_order::{AcceptanceCommand, WorkOrder, WorkOrderError};
