//! Snapshot of the last (or preview) chat-completion request so UIs can
//! show exactly what the model will see — messages, tool schemas, sizes.

use z_engine_provider::{ChatMessage, ChatRequest, ContentPart, ToolDef};

use super::LoopConfig;
use crate::context;

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
        }
    }
}

fn estimate_tokens(text: &str) -> u64 {
    text.len().div_ceil(4) as u64
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
}
