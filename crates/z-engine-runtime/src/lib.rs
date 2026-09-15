//! Provider- and UI-independent task supervision. The application supplies
//! observed boundaries; this crate decides whether another attempt is allowed.

mod supervisor;
mod types;

pub use supervisor::Supervisor;
pub use types::{
    Boundary, MAX_CONTINUATIONS, SupervisionAction, SupervisionError, SupervisionReport, WorkAction,
};
