//! Code checkpoints in a shadow git repository: snapshot the whole working
//! tree, list changes between snapshots, and restore one.

mod exclude;
mod restore;
mod shadow;

pub use restore::{ChangeKind, PathChange, RestoreReport};
pub use shadow::ShadowRepo;
