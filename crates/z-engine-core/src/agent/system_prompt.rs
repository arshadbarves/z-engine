//! L0 system-prefix assembly (rebuilt per request, cache-friendly).
//!
//! Static prose lives in `crate::context` (`BASE_INSTRUCTIONS`); this
//! module only combines it with the project root and AGENTS.md.

use z_engine_provider::ChatMessage;

use crate::context;

use super::LoopConfig;

/// L0 prefix message (system + AGENTS.md), rebuilt per request but
/// byte-stable across rounds unless AGENTS.md changes.
pub(super) fn l0_message(cfg: &LoopConfig) -> ChatMessage {
    ChatMessage::system(context::build_system_prompt(
        &cfg.project_root,
        context::load_agents_md(&cfg.project_root).as_deref(),
    ))
}
