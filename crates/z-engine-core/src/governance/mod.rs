//! Governance boundary: typed work orders binding an agent's goal to
//! evidence-backed writable paths, a pure prompt builder that assembles
//! model instructions deterministically from a pinned snapshot, the
//! fail-closed gate every guarded mutation must clear, and the
//! verification that must pass before a guarded run may call itself done.
//!
//! Split by reason to change: the port onto workspace evidence
//! (`evidence_view`), admission rules (`work_order`), what a verification
//! command may be (`acceptance`), the admitted order and its single-slot
//! store (`active`), bounded prompt assembly (`prompt`), mutation
//! authorization (`gate`), bounded subprocess execution (`command_run`),
//! the facts a completion is judged against (`plan`), what the working
//! tree actually holds (`snapshot`), reconciling those accounts
//! (`audit`), what a run has already been judged on (`turn_record`),
//! completion checks (`verify`), and what those checks proved
//! (`manifest`).

pub mod acceptance;
pub mod active;
pub mod evidence_view;
pub mod gate;
pub mod manifest;
pub mod plan;
pub mod prompt;
pub mod snapshot;
pub mod turn_record;
pub mod verify;
pub mod work_order;

mod audit;
mod command_run;

pub use acceptance::{AcceptanceError, CommandPolicy, SAFE_CARGO_SUBCOMMANDS};
pub use active::{ActiveWorkOrder, WorkOrderStore};
pub use evidence_view::EvidenceView;
pub use gate::{
    EvidenceState, GateDecision, GateEngine, GateFailure, LineRange, MutationRequest, RustFacts,
    SemanticEvidence, SemanticHealth, SymbolExtent, changed_line_range,
};
pub use manifest::{CheckOutcome, CheckStatus, ScopeBreach, Verdict, VerificationManifest};
pub use plan::{
    ChangeState, MutationRecord, PlanError, ReadWitness, VerificationPlan, WorkspaceChange,
};
pub use prompt::{PromptManifest, PromptOverflow, PromptSection, PromptSnapshot, build_prompt};
pub use snapshot::{SnapshotError, WorkspaceSnapshot};
pub use turn_record::{TurnRecord, TurnRecordUnavailable};
pub use verify::{LATEST_MANIFEST, Verification, VerificationRunner, write_manifest};
pub use work_order::{AcceptanceCommand, WorkOrder, WorkOrderError};
