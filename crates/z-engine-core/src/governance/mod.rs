//! Governance boundary: typed work orders binding an agent's goal to
//! evidence-backed writable paths, and a pure prompt builder that
//! assembles model instructions deterministically from a pinned
//! snapshot.
//!
//! Split by reason to change: the port onto workspace evidence
//! (`evidence_view`), admission rules (`work_order`), the admitted order
//! and its single-slot store (`active`), and bounded prompt assembly
//! (`prompt`).

pub mod active;
pub mod evidence_view;
pub mod prompt;
pub mod work_order;

pub use active::{ActiveWorkOrder, WorkOrderStore};
pub use evidence_view::EvidenceView;
pub use prompt::{PromptManifest, PromptOverflow, PromptSection, PromptSnapshot, build_prompt};
pub use work_order::{AcceptanceCommand, WorkOrder, WorkOrderError};
