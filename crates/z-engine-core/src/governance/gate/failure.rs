//! The refusal vocabulary. Display text reaches the model verbatim, so
//! every message names the next action that would clear it — a refusal
//! the model cannot act on is a stall, not a guardrail.

use std::path::PathBuf;

/// Why a mutation (or a guarded shell command) was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateFailure {
    #[error(
        "guarded mode: no work order is active — call set_work_order with the goal, \
         the paths you will change, the symbols you will touch, and the evidence ids \
         backing them, then retry"
    )]
    NoWorkOrder,
    #[error("guarded mode: {path} resolves outside the project root and can never be written")]
    OutsideRoot { path: PathBuf },
    #[error(
        "guarded mode: {path} is not in the active work order's scope ({allowed}) — \
         change a scoped file, or re-declare the order with set_work_order citing fresh \
         evidence for {path}"
    )]
    OutOfScope { path: PathBuf, allowed: String },
    #[error(
        "guarded mode: this run has no read evidence for {path} — call read_file on it, \
         then retry"
    )]
    NoEvidence { path: PathBuf },
    #[error(
        "guarded mode: {path} no longer matches the bytes you read — call read_file on \
         it again, then retry"
    )]
    StaleEvidence { path: PathBuf },
    #[error(
        "guarded mode: this change touches {path} lines {changed} but your evidence only \
         covers {covered} — read_file the lines you are changing, then retry"
    )]
    RangeNotCovered {
        path: PathBuf,
        changed: String,
        covered: String,
    },
    #[error(
        "guarded mode: the Rust semantic provider is unavailable ({reason}); Rust changes \
         stay blocked until it answers again"
    )]
    SemanticProviderUnavailable { reason: String },
    #[error(
        "guarded mode: the active work order names no target_symbols, so a Rust change \
         cannot be localized — re-declare it with set_work_order naming the symbols you \
         intend to change"
    )]
    NoTargetSymbol,
    #[error(
        "guarded mode: none of the work order's target symbols ({symbols}) are declared \
         in {path} — change the file where the symbol lives, or re-declare the order"
    )]
    UnresolvedTargetSymbol { symbols: String, path: PathBuf },
    #[error(
        "guarded mode: refusing to run `{command}` — its write set cannot be proven \
         before it runs; make changes with edit_file/write_file inside the work order's \
         scope"
    )]
    UnprovenWriteSet { command: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_messages_tell_the_model_how_to_recover() {
        assert!(
            GateFailure::NoWorkOrder
                .to_string()
                .contains("set_work_order")
        );
        assert!(
            GateFailure::StaleEvidence {
                path: PathBuf::from("src/lib.rs")
            }
            .to_string()
            .contains("read_file")
        );
        assert!(
            GateFailure::RangeNotCovered {
                path: PathBuf::from("src/lib.rs"),
                changed: "9-12".into(),
                covered: "1-10".into(),
            }
            .to_string()
            .contains("only covers 1-10")
        );
        assert!(
            GateFailure::UnprovenWriteSet {
                command: "rm -rf target".into()
            }
            .to_string()
            .contains("rm -rf target")
        );
    }
}
