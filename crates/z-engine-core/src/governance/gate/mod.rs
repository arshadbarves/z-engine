//! The mutation gate: the fail-closed decision that stands between a
//! guarded agent and the working tree.
//!
//! Everything here is pure. The gate takes a bundle of already-gathered
//! facts ([`MutationRequest`]) and returns a [`GateDecision`]; it never
//! touches the filesystem, hashes bytes, spawns a language server, or
//! parses Rust. Those answers are produced by the tools layer (which
//! owns canonical path identity, evidence freshness, and the semantic
//! provider) and handed in, so the rules can be exhaustively tested and
//! so `governance` keeps depending on nothing but `evidence`.
//!
//! The rules, in order, mirror the questions a reviewer would ask before
//! accepting a patch:
//!
//! 1. Is there a declared work order at all?
//! 2. Does the path resolve inside the repository, and is it in scope?
//! 3. Did this run actually read the bytes it is about to overwrite —
//!    and the *lines* it is about to change?
//! 4. For Rust source: is the semantic provider healthy, and does a
//!    *declared target symbol* really live in this file according to that
//!    provider? Tree-sitter may narrow the candidates; only the language
//!    server can authorize.
//!
//! Anything unproven blocks. [`GateDecision::NeedsEvidence`] marks the
//! subset a model can clear by reading; [`GateDecision::Fail`] marks the
//! rest.
//!
//! Split by reason to change: the model-facing refusal vocabulary
//! (`failure`), the facts the rules consume (`facts`), the patch rules
//! themselves (`engine`), Rust localization (`localize`), the shell rule
//! (`command`), and the pure line arithmetic that localizes a change
//! (`range`).

mod command;
mod engine;
mod facts;
mod failure;
mod localize;
mod range;

pub use engine::{GateDecision, GateEngine};
pub use facts::{
    EvidenceState, LineRange, MutationRequest, RustFacts, SemanticEvidence, SemanticHealth,
};
pub use failure::GateFailure;
pub use range::changed_line_range;
