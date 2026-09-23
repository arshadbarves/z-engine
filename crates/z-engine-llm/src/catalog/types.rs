//! Model metadata shared with the GUI, and id lookup.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Prices in USD per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelInfo {
    /// The id as its provider lists it; OpenRouter ids are `vendor/model`.
    pub id: String,
    pub name: String,
    /// models.dev provider id (`openrouter`, `anthropic`, `opencode`, ...).
    pub provider: String,
    /// Tokens; 0 when unknown.
    pub context_window: u32,
    /// Tokens; 0 when unknown.
    pub max_output: u32,
    pub pricing: Option<Pricing>,
    pub tools: bool,
    pub vision: bool,
    pub reasoning: bool,
}

/// Known models, in lookup-preference order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub models: Vec<ModelInfo>,
}

impl ModelCatalog {
    /// Find a model by exact id, then ignoring a `vendor/` prefix on either
    /// side, then case-insensitively. Earlier catalog entries win ties.
    pub fn lookup(&self, model_id: &str) -> Option<&ModelInfo> {
        let wanted = model_id.trim();
        if wanted.is_empty() {
            return None;
        }
        let bare = unprefixed(wanted);
        self.find(|id| id == wanted)
            .or_else(|| self.find(|id| unprefixed(id) == bare))
            .or_else(|| self.find(|id| id.eq_ignore_ascii_case(wanted)))
            .or_else(|| self.find(|id| unprefixed(id).eq_ignore_ascii_case(bare)))
    }

    fn find(&self, matches: impl Fn(&str) -> bool) -> Option<&ModelInfo> {
        self.models.iter().find(|model| matches(&model.id))
    }
}

/// `model` from `vendor/model`; ids without a vendor are returned whole.
fn unprefixed(id: &str) -> &str {
    id.split_once('/').map_or(id, |(_, rest)| rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(provider: &str, id: &str) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            name: id.into(),
            provider: provider.into(),
            context_window: 0,
            max_output: 0,
            pricing: None,
            tools: true,
            vision: false,
            reasoning: false,
        }
    }

    #[test]
    fn lookup_prefers_exact_then_unprefixed_then_case_insensitive() {
        let catalog = ModelCatalog {
            models: vec![
                model("openrouter", "anthropic/claude-sonnet-4.5"),
                model("openai", "gpt-4o"),
                model("reseller", "gpt-4o"),
            ],
        };
        let provider = |id: &str| catalog.lookup(id).map(|model| model.provider.as_str());
        assert_eq!(provider("gpt-4o"), Some("openai"));
        assert_eq!(provider("openai/gpt-4o"), Some("openai"));
        assert_eq!(provider("claude-sonnet-4.5"), Some("openrouter"));
        assert_eq!(provider("ANTHROPIC/Claude-Sonnet-4.5"), Some("openrouter"));
        assert_eq!(provider("OpenAI/GPT-4O"), Some("openai"));
        assert_eq!(provider("  "), None);
        assert_eq!(provider("gpt-5"), None);
    }

    #[test]
    fn serializes_camel_case_for_the_gui() {
        let mut info = model("anthropic", "claude-x");
        info.context_window = 200_000;
        info.pricing = Some(Pricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        });
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["contextWindow"], 200_000);
        assert_eq!(json["maxOutput"], 0);
        assert_eq!(json["pricing"]["cacheRead"], 0.3);
        assert_eq!(json["pricing"]["cacheWrite"], 3.75);
        let back: ModelInfo = serde_json::from_value(json).unwrap();
        assert_eq!(back, info);
    }
}
