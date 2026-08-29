//! Pure, deterministic prompt assembly from a pinned snapshot and
//! token budget.  No I/O, no global state — the caller captures the
//! snapshot once and passes it in.

use serde::Serialize;

/// Immutable snapshot of all data that feeds into a prompt. Callers
/// capture this once per turn; the prompt builder is a pure function
/// over `(&PromptSnapshot, budget)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromptSnapshot {
    /// L0 system instructions (rendered markdown).
    pub system_instructions: String,
    /// Active work-order digest (empty when no order is active).
    pub order_digest: String,
    /// Evidence excerpts relevant to the current order.
    pub evidence_excerpts: Vec<String>,
    /// Recent failure messages for retry context.
    pub recent_failures: Vec<String>,
    /// Working conversation messages (serialized).
    pub working_messages: Vec<String>,
    /// Tool definitions (name + schema, serialized).
    pub tool_defs: Vec<String>,
}

/// The assembled prompt manifest: sections in canonical order with a
/// token estimate that the caller can check against the provider's
/// context window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromptManifest {
    /// Ordered sections exactly as they will appear in the prompt.
    pub sections: Vec<PromptSection>,
    /// Estimated total tokens across all sections.
    pub estimated_tokens: u64,
}

/// One section of the assembled prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromptSection {
    pub label: String,
    pub content: String,
    pub estimated_tokens: u64,
}

/// Error when pinned (non-trimmable) content alone exceeds the budget.
#[derive(Debug, thiserror::Error)]
#[error("pinned content alone requires ~{required} tokens, exceeding budget of {budget}")]
pub struct PromptOverflow {
    pub required: u64,
    pub budget: u64,
}

fn estimate_tokens(text: &str) -> u64 {
    // ~4 bytes per token for code/English
    (text.len() as u64).div_ceil(4).max(if text.is_empty() { 0 } else { 1 })
}

/// Build a prompt manifest from a pinned snapshot and token budget.
///
/// This is a **pure function**: same inputs → same output, no I/O, no
/// side effects, no hidden global state.  Sections appear in a fixed
/// canonical order:
///
/// 1. System instructions (pinned)
/// 2. Order digest (pinned, if present)
/// 3. Evidence excerpts (pinned, if present)
/// 4. Recent failures (trimmable)
/// 5. Working messages (trimmable)
/// 6. Tool definitions (pinned)
///
/// Returns `Err(PromptOverflow)` when pinned content alone exceeds
/// `budget_tokens`.
pub fn build_prompt(
    snapshot: &PromptSnapshot,
    budget_tokens: u64,
) -> Result<PromptManifest, PromptOverflow> {
    let mut sections = Vec::new();

    // --- pinned sections ---
    let sys = make_section("system-instructions", &snapshot.system_instructions);
    sections.push(sys);

    if !snapshot.order_digest.is_empty() {
        sections.push(make_section("order-digest", &snapshot.order_digest));
    }

    for (i, excerpt) in snapshot.evidence_excerpts.iter().enumerate() {
        sections.push(make_section(&format!("evidence-{i}"), excerpt));
    }

    let mut tool_sections = Vec::new();
    for (i, def) in snapshot.tool_defs.iter().enumerate() {
        tool_sections.push(make_section(&format!("tool-{i}"), def));
    }

    let pinned_tokens: u64 = sections.iter().map(|s| s.estimated_tokens).sum::<u64>()
        + tool_sections.iter().map(|s| s.estimated_tokens).sum::<u64>();

    if pinned_tokens > budget_tokens {
        return Err(PromptOverflow {
            required: pinned_tokens,
            budget: budget_tokens,
        });
    }

    // --- trimmable sections (trim from oldest first if over budget) ---
    let mut remaining = budget_tokens - pinned_tokens;

    for (i, failure) in snapshot.recent_failures.iter().enumerate() {
        let sec = make_section(&format!("failure-{i}"), failure);
        if sec.estimated_tokens <= remaining {
            remaining -= sec.estimated_tokens;
            sections.push(sec);
        }
    }

    for (i, msg) in snapshot.working_messages.iter().enumerate() {
        let sec = make_section(&format!("working-{i}"), msg);
        if sec.estimated_tokens <= remaining {
            remaining -= sec.estimated_tokens;
            sections.push(sec);
        }
    }

    sections.extend(tool_sections);

    let estimated_tokens = sections.iter().map(|s| s.estimated_tokens).sum();
    Ok(PromptManifest {
        sections,
        estimated_tokens,
    })
}

fn make_section(label: &str, content: &str) -> PromptSection {
    PromptSection {
        label: label.to_string(),
        content: content.to_string(),
        estimated_tokens: estimate_tokens(content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot() -> PromptSnapshot {
        PromptSnapshot {
            system_instructions: "You are a coding agent.".to_string(),
            order_digest: "Fix bug in parser".to_string(),
            evidence_excerpts: vec!["fn parse() -> Result".to_string()],
            recent_failures: vec!["cargo test failed".to_string()],
            working_messages: vec!["user: fix the parser".to_string()],
            tool_defs: vec!["read_file: reads a file".to_string()],
        }
    }

    #[test]
    fn build_prompt_is_deterministic() {
        let snapshot = test_snapshot();
        let a = build_prompt(&snapshot, 25_000).unwrap();
        let b = build_prompt(&snapshot, 25_000).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn build_prompt_respects_budget() {
        let snapshot = test_snapshot();
        let manifest = build_prompt(&snapshot, 25_000).unwrap();
        assert!(manifest.estimated_tokens <= 25_000);
    }

    #[test]
    fn build_prompt_overflow_when_pinned_exceeds_budget() {
        let snapshot = test_snapshot();
        // Budget of 1 token should fail since system instructions alone need more.
        let err = build_prompt(&snapshot, 1).unwrap_err();
        assert!(err.required > 1);
        assert_eq!(err.budget, 1);
    }

    #[test]
    fn build_prompt_sections_in_canonical_order() {
        let snapshot = test_snapshot();
        let manifest = build_prompt(&snapshot, 25_000).unwrap();
        let labels: Vec<&str> = manifest.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "system-instructions",
                "order-digest",
                "evidence-0",
                "failure-0",
                "working-0",
                "tool-0",
            ]
        );
    }

    #[test]
    fn build_prompt_trims_working_messages_when_tight() {
        let mut snapshot = test_snapshot();
        // Add many large working messages
        for i in 0..100 {
            snapshot
                .working_messages
                .push(format!("message {i}: {}", "x".repeat(400)));
        }
        let manifest = build_prompt(&snapshot, 100).unwrap();
        // Should have fewer working messages than input due to trimming.
        let working_count = manifest
            .sections
            .iter()
            .filter(|s| s.label.starts_with("working-"))
            .count();
        assert!(working_count < 101);
        assert!(manifest.estimated_tokens <= 100);
    }

    #[test]
    fn empty_order_digest_omitted() {
        let mut snapshot = test_snapshot();
        snapshot.order_digest = String::new();
        let manifest = build_prompt(&snapshot, 25_000).unwrap();
        assert!(!manifest
            .sections
            .iter()
            .any(|s| s.label == "order-digest"));
    }
}
