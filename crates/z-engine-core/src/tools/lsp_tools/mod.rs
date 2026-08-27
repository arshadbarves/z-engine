//! LSP-backed tools: `go_to_definition`, `find_references`,
//! `lsp_diagnostics`, plus the post-edit diagnostics attachment used by the
//! loop's feedback hook (spec §9 v0.8).
//!
//! All tools degrade gracefully: when no LSP server is available for the
//! project they return a model-visible note instead of failing hard.
//!
//! Composition root: one file per tool plus shared LSP plumbing.

mod definition;
mod diagnostics;
mod helpers;
mod references;

pub use definition::GoToDefinitionTool;
pub use diagnostics::DiagnosticsTool;
pub(crate) use diagnostics::maybe_attach_diagnostics;
pub use references::FindReferencesTool;
