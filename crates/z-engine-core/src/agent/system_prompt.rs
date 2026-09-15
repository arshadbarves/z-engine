//! L0 system-prefix assembly (rebuilt per request, cache-friendly).
//!
//! Static prose lives in `crate::prompts`; this module combines methodology
//! and task protocols with the project root and repository instructions.

use z_engine_provider::ChatMessage;

use crate::context;

use super::LoopConfig;

/// L0 prefix message (system + AGENTS.md), rebuilt per request but
/// byte-stable across rounds unless AGENTS.md changes.
pub(super) fn l0_message(cfg: &LoopConfig) -> ChatMessage {
    let base = context::build_system_prompt(
        &cfg.project_root,
        context::load_agents_md(&cfg.project_root).as_deref(),
    );
    ChatMessage::system(format!(
        "{base}\n\n{}\n\n{}\n\n{}",
        crate::prompts::TASK_COMPLETION,
        crate::prompts::TASK_SUPERVISION,
        crate::prompts::CONTEXT_PACKET,
    ))
}
