//! Pure, deterministic prompt assembly from a pinned snapshot and a
//! token budget. No I/O, no global state, no clocks: the caller captures
//! the snapshot once and passes it in, so the same snapshot always yields
//! byte-identical output.
//!
//! Token counting reuses the one canonical estimator
//! ([`crate::context::budget::estimate_tokens`]) so a manifest can be
//! compared with the loop's own pressure numbers.

use serde::Serialize;

use crate::context::budget::estimate_tokens;

/// Immutable snapshot of everything that feeds one prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromptSnapshot {
    /// L0 system instructions (rendered markdown).
    pub system_instructions: String,
    /// Active work-order digest (empty when no order is active).
    pub order_digest: String,
    /// Evidence excerpts backing the active order.
    pub evidence_excerpts: Vec<String>,
    /// Recent failure messages, oldest first.
    pub recent_failures: Vec<String>,
    /// Working conversation messages, oldest first.
    pub working_messages: Vec<String>,
    /// Tool definitions (name + description + schema).
    pub tool_defs: Vec<String>,
}

/// The assembled prompt: sections in canonical order plus the estimated
/// total the caller can check against the provider's context window.
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
    /// Stable identity of the section (`system-instructions`,
    /// `working-7`, `working-omitted`, …); indices are the snapshot's, so
    /// a gap is visible when older content was trimmed.
    pub label: String,
    /// Exact text this section contributes to the prompt.
    pub content: String,
    /// Estimated tokens for `content`.
    pub estimated_tokens: u64,
}

/// Pinned (non-trimmable) content alone exceeds the budget.
#[derive(Debug, thiserror::Error)]
#[error("pinned content alone requires ~{required} tokens, exceeding budget of {budget}")]
pub struct PromptOverflow {
    pub required: u64,
    pub budget: u64,
}

/// Build a prompt manifest from a pinned snapshot and a token budget.
///
/// Sections appear in a fixed canonical order:
///
/// 1. System instructions (pinned)
/// 2. Order digest (pinned, when an order is active)
/// 3. Evidence excerpts (pinned)
/// 4. Recent failures (trimmed oldest-first)
/// 5. Working messages (trimmed oldest-first)
/// 6. Tool definitions (pinned)
///
/// Trimmed groups keep the **newest contiguous** run that fits — never a
/// scattered subset — and, when anything was dropped, carry an explicit
/// omission marker so the model can see that history is missing rather
/// than silently reading a doctored transcript.
///
/// Returns [`PromptOverflow`] when pinned content alone exceeds
/// `budget_tokens`.
pub fn build_prompt(
    snapshot: &PromptSnapshot,
    budget_tokens: u64,
) -> Result<PromptManifest, PromptOverflow> {
    let mut sections = vec![make_section(
        "system-instructions",
        &snapshot.system_instructions,
    )];
    if !snapshot.order_digest.is_empty() {
        sections.push(make_section("order-digest", &snapshot.order_digest));
    }
    for (i, excerpt) in snapshot.evidence_excerpts.iter().enumerate() {
        sections.push(make_section(&format!("evidence-{i}"), excerpt));
    }
    let tool_sections: Vec<PromptSection> = snapshot
        .tool_defs
        .iter()
        .enumerate()
        .map(|(i, def)| make_section(&format!("tool-{i}"), def))
        .collect();

    let pinned_tokens = total_tokens(&sections).saturating_add(total_tokens(&tool_sections));
    if pinned_tokens > budget_tokens {
        return Err(PromptOverflow {
            required: pinned_tokens,
            budget: budget_tokens,
        });
    }

    let mut remaining = budget_tokens - pinned_tokens;
    remaining = push_recent(
        &mut sections,
        &snapshot.recent_failures,
        "failure",
        "earlier failures",
        remaining,
    );
    push_recent(
        &mut sections,
        &snapshot.working_messages,
        "working",
        "earlier messages",
        remaining,
    );
    sections.extend(tool_sections);

    let estimated_tokens = total_tokens(&sections);
    Ok(PromptManifest {
        sections,
        estimated_tokens,
    })
}

/// Append the newest contiguous run of `items` that fits in `budget`,
/// preceded by an omission marker when older items were dropped. Returns
/// the budget left over.
fn push_recent(
    sections: &mut Vec<PromptSection>,
    items: &[String],
    label: &str,
    noun: &str,
    budget: u64,
) -> u64 {
    let mut start = items.len();
    let mut used = 0u64;
    for (i, item) in items.iter().enumerate().rev() {
        let cost = estimate_tokens(item);
        // Accepting `i` leaves `i` older items omitted, and the marker
        // announcing them costs tokens too.
        let marker = (i > 0).then(|| estimate_tokens(&omission_text(i, noun)));
        if used + cost + marker.unwrap_or(0) > budget {
            break;
        }
        used += cost;
        start = i;
    }
    if start > 0 {
        let marker = make_section(&format!("{label}-omitted"), &omission_text(start, noun));
        used += marker.estimated_tokens;
        sections.push(marker);
    }
    for (i, item) in items.iter().enumerate().skip(start) {
        sections.push(make_section(&format!("{label}-{i}"), item));
    }
    budget.saturating_sub(used)
}

fn omission_text(count: usize, noun: &str) -> String {
    format!("[{count} {noun} omitted to fit the context budget]")
}

fn total_tokens(sections: &[PromptSection]) -> u64 {
    sections.iter().map(|s| s.estimated_tokens).sum()
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

    fn labels(manifest: &PromptManifest) -> Vec<&str> {
        manifest.sections.iter().map(|s| s.label.as_str()).collect()
    }

    #[test]
    fn build_prompt_is_deterministic() {
        assert_eq!(
            build_prompt(&test_snapshot(), 25_000).unwrap(),
            build_prompt(&test_snapshot(), 25_000).unwrap()
        );
    }

    #[test]
    fn manifest_serializes_byte_identically_for_equal_snapshots() {
        let a = serde_json::to_vec(&build_prompt(&test_snapshot(), 25_000).unwrap()).unwrap();
        let b = serde_json::to_vec(&build_prompt(&test_snapshot(), 25_000).unwrap()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn manifest_serialization_matches_pinned_bytes() {
        let snapshot = PromptSnapshot {
            system_instructions: "sys".into(),
            order_digest: String::new(),
            evidence_excerpts: vec![],
            recent_failures: vec![],
            working_messages: vec!["hi".into()],
            tool_defs: vec!["t".into()],
        };
        let json = serde_json::to_string(&build_prompt(&snapshot, 100).unwrap()).unwrap();
        assert_eq!(
            json,
            r#"{"sections":[{"label":"system-instructions","content":"sys","estimated_tokens":1},{"label":"working-0","content":"hi","estimated_tokens":1},{"label":"tool-0","content":"t","estimated_tokens":1}],"estimated_tokens":3}"#
        );
    }

    #[test]
    fn build_prompt_respects_budget() {
        let manifest = build_prompt(&test_snapshot(), 25_000).unwrap();
        assert!(manifest.estimated_tokens <= 25_000);
        assert_eq!(
            manifest.estimated_tokens,
            manifest
                .sections
                .iter()
                .map(|s| s.estimated_tokens)
                .sum::<u64>()
        );
    }

    #[test]
    fn build_prompt_overflow_when_pinned_exceeds_budget() {
        let err = build_prompt(&test_snapshot(), 1).unwrap_err();
        assert!(err.required > 1);
        assert_eq!(err.budget, 1);
    }

    #[test]
    fn build_prompt_sections_in_canonical_order() {
        let manifest = build_prompt(&test_snapshot(), 25_000).unwrap();
        assert_eq!(
            labels(&manifest),
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
    fn trimming_keeps_the_newest_contiguous_messages() {
        let mut snapshot = test_snapshot();
        snapshot.recent_failures.clear();
        snapshot.working_messages = (0..10)
            .map(|i| format!("m{i}: {}", "x".repeat(36)))
            .collect();
        // Pinned content is ~17 tokens; leave room for ~3 messages.
        let manifest = build_prompt(&snapshot, 60).unwrap();

        let kept: Vec<&str> = labels(&manifest)
            .into_iter()
            .filter(|l| l.starts_with("working-"))
            .collect();
        assert!(kept.len() > 1 && kept.len() < 11, "kept: {kept:?}");
        // The marker comes first, then an unbroken run ending at the newest.
        assert_eq!(kept[0], "working-omitted");
        let indices: Vec<usize> = kept[1..]
            .iter()
            .map(|l| l.trim_start_matches("working-").parse().unwrap())
            .collect();
        assert_eq!(*indices.last().unwrap(), 9, "newest message must survive");
        assert!(
            indices.windows(2).all(|w| w[1] == w[0] + 1),
            "trimmed run must be contiguous: {indices:?}"
        );
        // The marker names exactly how many older messages are missing.
        let marker = manifest
            .sections
            .iter()
            .find(|s| s.label == "working-omitted")
            .unwrap();
        assert_eq!(
            marker.content,
            format!(
                "[{} earlier messages omitted to fit the context budget]",
                indices[0]
            )
        );
        assert!(manifest.estimated_tokens <= 60);
    }

    #[test]
    fn nothing_omitted_means_no_marker() {
        let manifest = build_prompt(&test_snapshot(), 25_000).unwrap();
        assert!(!labels(&manifest).iter().any(|l| l.ends_with("-omitted")));
    }

    #[test]
    fn empty_order_digest_omitted() {
        let mut snapshot = test_snapshot();
        snapshot.order_digest = String::new();
        let manifest = build_prompt(&snapshot, 25_000).unwrap();
        assert!(!labels(&manifest).contains(&"order-digest"));
    }
}
