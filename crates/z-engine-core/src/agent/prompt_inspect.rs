//! Snapshot of the last (or preview) chat-completion request so UIs can
//! show exactly what the model will see — messages, tool schemas, sizes.
//!
//! The same snapshot also feeds governance's bounded prompt view
//! ([`crate::governance::build_prompt`]): [`PromptInspect::with_manifest`]
//! turns the request into a [`PromptSnapshot`] and records the resulting
//! [`PromptManifest`], so a guarded run can show which sections are
//! pinned, which were trimmed, and what the whole thing is estimated to
//! cost — measured with the loop's own token estimator.

use z_engine_provider::{ChatMessage, ChatRequest, ContentPart, ToolDef};

use super::LoopConfig;
use crate::context::{self, budget::estimate_tokens};
use crate::governance::{ActiveWorkOrder, PromptManifest, PromptSnapshot, build_prompt};

/// Label given to the pinned work-order digest (guarded runs only).
const WORK_ORDER_LABEL: &str = "Work order";

/// One message that was (or will be) sent on the wire.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptPart {
    pub role: String,
    pub label: String,
    pub content: String,
    pub tokens: u64,
}

/// One tool definition advertised alongside the messages.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptTool {
    pub name: String,
    pub description: String,
    pub schema: String,
    pub tokens: u64,
}

/// Structured view of a `ChatRequest` for the prompt inspector.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptInspect {
    pub model: String,
    /// True when this snapshot was actually posted to the provider.
    pub sent: bool,
    pub messages: Vec<PromptPart>,
    pub tools: Vec<PromptTool>,
    pub total_tokens: u64,
    /// Governance's bounded, canonical-order view of the same prompt.
    /// `None` until [`PromptInspect::with_manifest`] is called, and when
    /// pinned content alone exceeds the budget.
    pub manifest: Option<PromptManifest>,
}

impl PromptInspect {
    /// L0 + advertised tools, for inspect-before-first-turn.
    pub fn preview(cfg: &LoopConfig, tools: Vec<ToolDef>) -> Self {
        let l0 = ChatMessage::system(context::build_system_prompt(
            &cfg.project_root,
            context::load_agents_md(&cfg.project_root).as_deref(),
        ));
        Self::from_request(
            &ChatRequest::new(cfg.model.clone(), vec![l0]).with_tools(tools),
            false,
        )
    }

    /// Resume snapshot: L0 + persisted working set (notes/MCP filled in later).
    pub fn resumed(cfg: &LoopConfig, working: &[ChatMessage], tools: Vec<ToolDef>) -> Self {
        let mut messages = vec![ChatMessage::system(context::build_system_prompt(
            &cfg.project_root,
            context::load_agents_md(&cfg.project_root).as_deref(),
        ))];
        messages.extend(working.iter().cloned());
        Self::from_request(
            &ChatRequest::new(cfg.model.clone(), messages).with_tools(tools),
            true,
        )
    }

    pub fn from_request(req: &ChatRequest, sent: bool) -> Self {
        let messages: Vec<PromptPart> = req
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| part_from_message(i, m))
            .collect();
        let tools: Vec<PromptTool> = req.tools.iter().map(tool_from_def).collect();
        let total_tokens = messages.iter().map(|m| m.tokens).sum::<u64>()
            + tools.iter().map(|t| t.tokens).sum::<u64>();
        Self {
            model: req.model.clone(),
            sent,
            messages,
            tools,
            total_tokens,
            manifest: None,
        }
    }

    /// Attach the governance manifest for this prompt, optionally pinning
    /// `order`'s digest and backing-evidence excerpts. Pure: same inputs
    /// produce the same manifest. Overflow (pinned content alone exceeding
    /// `budget_tokens`) leaves the manifest empty and is logged — the
    /// inspector must never truncate what the loop actually sent.
    pub fn with_manifest(mut self, order: Option<&ActiveWorkOrder>, budget_tokens: u64) -> Self {
        match build_prompt(&self.snapshot(order), budget_tokens) {
            Ok(manifest) => self.manifest = Some(manifest),
            Err(overflow) => tracing::warn!(%overflow, "prompt manifest over budget"),
        }
        self
    }

    /// Split the inspected request into governance's sections: the leading
    /// system prefix is pinned instruction text, an active order carries
    /// its own digest and evidence excerpts, and everything after the
    /// prefix is trimmable conversation.
    fn snapshot(&self, order: Option<&ActiveWorkOrder>) -> PromptSnapshot {
        let prefix_len = self
            .messages
            .iter()
            .take_while(|m| m.role == "system")
            .count();
        let system_instructions = self.messages[..prefix_len]
            .iter()
            .filter(|m| m.label != WORK_ORDER_LABEL)
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
        PromptSnapshot {
            system_instructions,
            order_digest: order.map(ActiveWorkOrder::digest).unwrap_or_default(),
            evidence_excerpts: order
                .map(ActiveWorkOrder::evidence_excerpts)
                .unwrap_or_default(),
            // Failures are surfaced to the model as ordinary tool results,
            // so they arrive inside the working messages below.
            recent_failures: Vec::new(),
            working_messages: self.messages[prefix_len..]
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect(),
            tool_defs: self
                .tools
                .iter()
                .map(|t| format!("{}\n{}\n{}", t.name, t.description, t.schema))
                .collect(),
        }
    }
}

fn part_from_message(idx: usize, msg: &ChatMessage) -> PromptPart {
    let (role, content) = match msg {
        ChatMessage::System { content } => ("system", content.clone()),
        ChatMessage::User { content } => ("user", content.clone()),
        ChatMessage::UserMulti { content } => ("user", flatten_parts(content)),
        ChatMessage::Assistant {
            content,
            tool_calls,
        } => {
            let mut body = content.clone().unwrap_or_default();
            for call in tool_calls {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(&format!(
                    "[tool_call {} {}] {}",
                    call.id, call.function.name, call.function.arguments
                ));
            }
            ("assistant", body)
        }
        ChatMessage::Tool {
            tool_call_id,
            content,
        } => ("tool", format!("[{tool_call_id}]\n{content}")),
    };
    PromptPart {
        label: label_for(idx, role, &content),
        role: role.to_string(),
        tokens: estimate_tokens(&content),
        content,
    }
}

fn flatten_parts(parts: &[ContentPart]) -> String {
    let mut out = String::new();
    for part in parts {
        if !out.is_empty() {
            out.push('\n');
        }
        match part {
            ContentPart::Text { text } => out.push_str(text),
            ContentPart::ImageUrl { .. } => out.push_str("[image]"),
        }
    }
    out
}

fn label_for(idx: usize, role: &str, content: &str) -> String {
    match role {
        "system" if content.starts_with("# Repository symbol map") => "Repo map".into(),
        "system" if content.starts_with("# Active work order") => WORK_ORDER_LABEL.into(),
        "system" if content.starts_with("# Session context notes") => "Notes".into(),
        "system" if idx == 0 => "System".into(),
        "system" => "System".into(),
        "user" => "User".into(),
        "assistant" => "Assistant".into(),
        "tool" => "Tool result".into(),
        other => other.to_string(),
    }
}

fn tool_from_def(def: &ToolDef) -> PromptTool {
    let schema = serde_json::to_string_pretty(&def.function.parameters).unwrap_or_default();
    let blob = format!(
        "{}\n{}\n{schema}",
        def.function.name, def.function.description
    );
    PromptTool {
        name: def.function.name.clone(),
        description: def.function.description.clone(),
        schema,
        tokens: estimate_tokens(&blob),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z_engine_provider::{ChatMessage, ChatRequest, ToolDef};

    #[test]
    fn labels_l0_repo_map_notes_and_user() {
        let req = ChatRequest::new(
            "test-model",
            vec![
                ChatMessage::system("You are an autonomous coding agent."),
                ChatMessage::system("# Repository symbol map (definition lines)\nsrc/lib.rs"),
                ChatMessage::system(
                    "# Session context notes (authoritative; survives compaction)\n- hi",
                ),
                ChatMessage::user("reduce the prompt"),
            ],
        );
        let snap = PromptInspect::from_request(&req, true);
        assert!(snap.sent);
        assert_eq!(snap.model, "test-model");
        let labels: Vec<_> = snap.messages.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(labels, ["System", "Repo map", "Notes", "User"]);
        assert!(snap.total_tokens > 0);
        assert_eq!(
            snap.total_tokens,
            snap.messages.iter().map(|m| m.tokens).sum::<u64>()
        );
    }

    #[test]
    fn tools_are_counted_separately() {
        let def = ToolDef::function(
            "read_file",
            "Read a file",
            serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        );
        let req = ChatRequest::new("m", vec![ChatMessage::user("hi")]).with_tools(vec![def]);
        let snap = PromptInspect::from_request(&req, false);
        assert!(!snap.sent);
        assert_eq!(snap.tools.len(), 1);
        assert_eq!(snap.tools[0].name, "read_file");
        assert!(snap.tools[0].schema.contains("path"));
        assert!(snap.total_tokens > snap.messages[0].tokens);
    }

    #[test]
    fn manifest_is_absent_until_requested_and_pins_the_order_when_present() {
        let req = ChatRequest::new(
            "m",
            vec![
                ChatMessage::system("L0 instructions"),
                ChatMessage::system("# Active work order (guarded)\nid: wo-1\n"),
                ChatMessage::user("go"),
            ],
        );
        let bare = PromptInspect::from_request(&req, true);
        assert!(bare.manifest.is_none(), "manifest must be opt-in");
        assert_eq!(bare.messages[1].label, WORK_ORDER_LABEL);

        // Without an order, no order/evidence sections exist.
        let plain = bare.clone().with_manifest(None, 10_000);
        let labels = section_labels(&plain);
        assert!(!labels.iter().any(|l| l.starts_with("order-digest")));
        assert!(!labels.iter().any(|l| l.starts_with("evidence-")));

        // With one, the digest and its evidence excerpts are pinned, and the
        // digest is never double-counted as system instructions.
        let order = active_order();
        let guarded = bare.with_manifest(Some(&order), 10_000);
        let manifest = guarded.manifest.as_ref().unwrap();
        assert!(section_labels(&guarded).contains(&"order-digest".to_string()));
        assert!(section_labels(&guarded).contains(&"evidence-0".to_string()));
        let system = manifest
            .sections
            .iter()
            .find(|s| s.label == "system-instructions")
            .unwrap();
        assert_eq!(system.content, "L0 instructions");
        assert!(manifest.estimated_tokens > 0);
    }

    #[test]
    fn manifest_is_deterministic_and_omitted_when_pinned_content_overflows() {
        let req = ChatRequest::new(
            "m",
            vec![ChatMessage::system("L0"), ChatMessage::user("go")],
        );
        let snap = PromptInspect::from_request(&req, true);
        let order = active_order();
        let a = snap.clone().with_manifest(Some(&order), 10_000);
        let b = snap.clone().with_manifest(Some(&order), 10_000);
        assert_eq!(a.manifest, b.manifest, "manifest must be pure");
        assert!(
            snap.with_manifest(Some(&order), 1).manifest.is_none(),
            "over-budget prompts must not be misreported"
        );
    }

    fn section_labels(inspect: &PromptInspect) -> Vec<String> {
        inspect
            .manifest
            .as_ref()
            .map(|m| m.sections.iter().map(|s| s.label.clone()).collect())
            .unwrap_or_default()
    }

    fn active_order() -> ActiveWorkOrder {
        let record = crate::evidence::EvidenceRecord::new(
            "src/lib.rs",
            Some((1, 3)),
            "0".repeat(64),
            crate::evidence::BlobHandle::of(b"fn parse() {}"),
            "read_file",
            "working-tree",
        );
        ActiveWorkOrder::for_test(
            crate::governance::WorkOrder {
                id: "wo-1".into(),
                goal: "make parse fallible".into(),
                writable_paths: vec!["src/lib.rs".into()],
                target_symbols: vec!["parse".into()],
                evidence_ids: vec![record.id.clone()],
                acceptance_commands: Vec::new(),
            },
            vec![record],
        )
    }
}
