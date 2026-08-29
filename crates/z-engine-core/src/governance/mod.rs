//! Governance boundary: typed work orders that bind agent goals to
//! evidence-backed writable paths, and a pure prompt builder that
//! assembles model instructions deterministically from a pinned
//! snapshot.
//!
//! Work-order validation is deliberately separate from prompt
//! assembly so each concern can be tested and evolved independently.

pub mod prompt;
pub mod work_order;

pub use prompt::{PromptManifest, PromptOverflow, PromptSection, PromptSnapshot, build_prompt};
pub use work_order::{AcceptanceCommand, ActiveWorkOrder, WorkOrder, WorkOrderError};
