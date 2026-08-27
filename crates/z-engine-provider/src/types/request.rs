use serde::Serialize;

use super::tools::{ToolCall, ToolDef};

/// A chat completion request. Always sent with `stream: true`.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDef>,
    pub stream: bool,
    pub stream_options: StreamOptions,
    /// Explicit output ceiling. Without it, gateways like OpenRouter assume
    /// the model maximum (e.g. 65536) and pre-validate credits against
    /// prompt + max_tokens — failing near-zero balances even for tiny
    /// prompts ("can only afford N tokens").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Unified reasoning control (`{"effort": "low|medium|high|xhigh"}`).
    /// Only sent when the user explicitly picks an effort; omitted
    /// otherwise so non-reasoning models never see the parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningParam>,
}

/// Wire shape of the unified `reasoning` parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReasoningParam {
    pub effort: String,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            max_tokens: None,
            reasoning: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolDef>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning = Some(ReasoningParam {
            effort: effort.into(),
        });
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct StreamOptions {
    include_usage: bool,
}

/// A conversation message, tagged by role.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    /// Multimodal user turn (vision): text plus image parts, serialized as
    /// the OpenAI content-parts array. Same `role:"user"` tag as `User`.
    #[serde(rename = "user")]
    UserMulti {
        content: Vec<ContentPart>,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

/// One content part of a multimodal user message.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrlBody },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageUrlBody {
    /// Data URL (`data:image/png;base64,...`) or remote https URL.
    pub url: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        ChatMessage::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        ChatMessage::User {
            content: content.into(),
        }
    }

    /// User turn with attached images (data URLs). Plain text when the
    /// list is empty.
    pub fn user_with_images(content: impl Into<String>, images: &[String]) -> Self {
        if images.is_empty() {
            return Self::user(content);
        }
        let mut parts: Vec<ContentPart> = vec![ContentPart::Text {
            text: content.into(),
        }];
        for url in images {
            parts.push(ContentPart::ImageUrl {
                image_url: ImageUrlBody { url: url.clone() },
            });
        }
        ChatMessage::UserMulti { content: parts }
    }

    pub fn assistant_text(content: impl Into<String>) -> Self {
        ChatMessage::Assistant {
            content: Some(content.into()),
            tool_calls: Vec::new(),
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ChatMessage::Tool {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vision_message_serializes_content_parts_array() {
        let msg = ChatMessage::user_with_images(
            "what is this?",
            &["data:image/png;base64,AAAA".to_string()],
        );
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""type":"image_url""#));
        assert!(json.contains(r#""url":"data:image/png;base64,AAAA""#));

        // Plain submit must stay a string-content user message.
        let plain = ChatMessage::user_with_images("hi", &[]);
        let json = serde_json::to_string(&plain).unwrap();
        assert!(json.contains(r#""content":"hi""#));
        assert!(!json.contains("image_url"));
    }

    #[test]
    fn optional_params_omitted_when_unset() {
        let req = ChatRequest::new("m", vec![ChatMessage::user("x")]);
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("reasoning"));

        let req = req.with_max_tokens(4096).with_reasoning_effort("medium");
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""max_tokens":4096"#));
        assert!(json.contains(r#""reasoning":{"effort":"medium"}"#));
    }
}
