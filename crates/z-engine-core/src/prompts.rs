/// Static LLM prompt templates, one markdown file per prompt under
/// `prompts/`. Embedded at compile time; edit the markdown, not Rust.
///
/// To add a prompt: create `prompts/<name>.md`, add a `pub const <NAME>`
/// here, and reference it from call sites. Never inline prompt prose in
/// logic files.
///
/// L0 operating instructions — the provider-cache-friendly static prefix.
pub const SYSTEM_MAIN: &str = include_str!("../prompts/system-main.md");

/// Post-edit reviewer persona (spec section 9 v0.9).
pub const REVIEWER: &str = include_str!("../prompts/reviewer.md");

/// Compaction summarizer persona (spec section 6).
pub const SUMMARIZER: &str = include_str!("../prompts/summarizer.md");

/// Research sub-agent persona (spec section 9 v0.7).
pub const SUBAGENT: &str = include_str!("../prompts/subagent.md");

/// Short session-title generator (sidebar labels).
pub const SESSION_TITLE: &str = include_str!("../prompts/session-title.md");
