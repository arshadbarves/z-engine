//! Assembling one request from the turn's materials.
//!
//! Two assemblies, deliberately different, because guarded and unguarded
//! runs make different promises:
//!
//! - [`bounded`] is the guarded one. The pure prompt builder
//!   ([`crate::governance::build_prompt`]) runs *first* and its manifest
//!   decides what reaches the wire: which conversation messages survive,
//!   what the omission marker says, and — when the pinned content alone
//!   will not fit — that there is no request to send at all. Invariant 7
//!   is therefore enforced rather than observed: a
//!   [`PromptOverflow`] refuses the turn before the provider is called.
//! - [`legacy`] is the unguarded one: byte for byte the request this loop
//!   built before governance existed, with the manifest attached for the
//!   inspector only. Nothing an unguarded user sees changes.
//!
//! The point of doing it here rather than in `turn` is that there is only
//! ever *one* prompt architecture. The manifest's sections and the wire
//! messages are two renderings of the same decision, produced together
//! and from the same strings (see
//! [`super::prompt_inspect::role_and_content`]), so a manifest can never
//! describe a prompt nobody sent.

use z_engine_provider::{ChatMessage, ToolDef};

use crate::governance::{
    ActiveWorkOrder, PromptManifest, PromptOverflow, PromptSnapshot, build_prompt,
};

use super::prompt_inspect::{role_and_content, tool_blob};

/// Header for the pinned evidence block. Present on the wire exactly when
/// the manifest pins it, so the two cannot drift.
const EVIDENCE_HEADER: &str = "# Evidence backing the active work order";

/// Everything one request is built from.
pub(super) struct Materials<'a> {
    /// The harness's own system prefix — L0, repo map, notes — in order.
    pub(super) instructions: Vec<String>,
    /// The order in force, whose digest and evidence are pinned.
    pub(super) order: Option<&'a ActiveWorkOrder>,
    /// The conversation, oldest first.
    pub(super) working: &'a [ChatMessage],
    pub(super) tools: &'a [ToolDef],
    pub(super) budget_tokens: u64,
}

/// An assembled request, and the manifest describing it.
#[derive(Debug)]
pub(super) struct PromptPlan {
    pub(super) messages: Vec<ChatMessage>,
    /// How many leading messages the harness assembled itself — what a
    /// replay compares prompts on.
    pub(super) prefix_len: usize,
    /// `None` only in an unguarded run whose pinned content overflows,
    /// where the manifest is a report and not a gate.
    pub(super) manifest: Option<PromptManifest>,
}

/// Guarded assembly: the manifest is the request.
///
/// Returns [`PromptOverflow`] when the pinned content alone exceeds the
/// budget. The caller must treat that as a refusal — sending anyway would
/// mean the harness knowingly posted a prompt it had already judged
/// impossible to bound.
pub(super) fn bounded(materials: &Materials<'_>) -> Result<PromptPlan, PromptOverflow> {
    let pinned = Pinned::of(materials);
    let manifest = build_prompt(&snapshot(materials, &pinned), materials.budget_tokens)?;

    let mut messages = pinned.messages();
    if let Some(marker) = section_content(&manifest, "working-omitted") {
        messages.push(ChatMessage::system(marker));
    }
    let prefix_len = messages.len();
    messages.extend(
        materials.working[retained_from(&manifest, materials.working)..]
            .iter()
            .cloned(),
    );
    Ok(PromptPlan {
        messages,
        prefix_len,
        manifest: Some(manifest),
    })
}

/// Unguarded assembly: the request this loop has always built, with a
/// manifest attached for the inspector. An overflow is logged, never a
/// refusal — an unguarded run promised no bound and must not acquire one.
pub(super) fn legacy(materials: &Materials<'_>) -> PromptPlan {
    let pinned = Pinned::of(materials);
    let mut messages = pinned.instructions_and_digest();
    let prefix_len = messages.len();
    messages.extend(materials.working.iter().cloned());
    let manifest = match build_prompt(&snapshot(materials, &pinned), materials.budget_tokens) {
        Ok(manifest) => Some(manifest),
        Err(overflow) => {
            tracing::warn!(%overflow, "prompt manifest over budget");
            None
        }
    };
    PromptPlan {
        messages,
        prefix_len,
        manifest,
    }
}

/// The pinned text, rendered once and used for both the snapshot the
/// builder measures and the messages that go on the wire.
struct Pinned {
    instructions: Vec<String>,
    order_digest: String,
    evidence: String,
}

impl Pinned {
    fn of(materials: &Materials<'_>) -> Self {
        let excerpts = materials
            .order
            .map(ActiveWorkOrder::evidence_excerpts)
            .unwrap_or_default();
        Self {
            instructions: materials.instructions.clone(),
            order_digest: materials
                .order
                .map(ActiveWorkOrder::digest)
                .unwrap_or_default(),
            evidence: if excerpts.is_empty() {
                String::new()
            } else {
                let mut block = String::from(EVIDENCE_HEADER);
                for excerpt in &excerpts {
                    block.push_str("\n- ");
                    block.push_str(excerpt);
                }
                block
            },
        }
    }

    /// Instructions plus the order digest — the unguarded prefix, which
    /// pins no evidence block because unguarded runs mint no evidence.
    fn instructions_and_digest(&self) -> Vec<ChatMessage> {
        let mut out: Vec<ChatMessage> = self
            .instructions
            .iter()
            .cloned()
            .map(ChatMessage::system)
            .collect();
        if !self.order_digest.is_empty() {
            out.push(ChatMessage::system(self.order_digest.clone()));
        }
        out
    }

    /// The whole pinned prefix, evidence included.
    fn messages(&self) -> Vec<ChatMessage> {
        let mut out = self.instructions_and_digest();
        if !self.evidence.is_empty() {
            out.push(ChatMessage::system(self.evidence.clone()));
        }
        out
    }
}

/// The snapshot the builder measures — the *same* strings the wire
/// carries, so a section's estimate is an estimate of what was sent.
fn snapshot(materials: &Materials<'_>, pinned: &Pinned) -> PromptSnapshot {
    PromptSnapshot {
        system_instructions: pinned.instructions.join("\n\n"),
        order_digest: pinned.order_digest.clone(),
        evidence_excerpts: if pinned.evidence.is_empty() {
            Vec::new()
        } else {
            vec![pinned.evidence.clone()]
        },
        // Failures reach the model as ordinary tool results, so they are
        // already among the working messages below.
        recent_failures: Vec::new(),
        working_messages: materials.working.iter().map(rendered).collect(),
        tool_defs: materials.tools.iter().map(tool_blob).collect(),
    }
}

fn rendered(msg: &ChatMessage) -> String {
    let (role, content) = role_and_content(msg);
    format!("{role}: {content}")
}

fn section_content(manifest: &PromptManifest, label: &str) -> Option<String> {
    manifest
        .sections
        .iter()
        .find(|s| s.label == label)
        .map(|s| s.content.clone())
}

/// The first conversation message the manifest kept.
///
/// Two corrections on top of the manifest's own answer, both of which can
/// only drop *more*:
///
/// - a manifest with no `working-*` section kept nothing;
/// - a kept run must not begin with a tool result whose assistant message
///   was trimmed away. Strict OpenAI-compatible providers reject an
///   unpaired tool result with a 400, which would poison the session.
fn retained_from(manifest: &PromptManifest, working: &[ChatMessage]) -> usize {
    let mut start = manifest
        .sections
        .iter()
        .filter_map(|s| s.label.strip_prefix("working-"))
        .filter_map(|i| i.parse::<usize>().ok())
        .min()
        .unwrap_or(working.len());
    while start < working.len() && matches!(working[start], ChatMessage::Tool { .. }) {
        start += 1;
    }
    start
}

#[cfg(test)]
mod tests;
