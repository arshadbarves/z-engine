//! Cost estimation (spec §9 v1.0 calibration).
//!
//! Pricing is USD per million tokens. Known prefixes cover the common
//! OpenRouter families; anything unknown falls back to the configured
//! default so the status bar stays honest (`$–` when truly unknowable).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pricing {
    pub usd_per_mtok_input: f64,
    pub usd_per_mtok_output: f64,
}

/// Ordered (model-substring, pricing) table — first match wins.
pub fn for_model(model: &str) -> Option<Pricing> {
    let m = model.to_lowercase();
    let p = |i: f64, o: f64| {
        Some(Pricing {
            usd_per_mtok_input: i,
            usd_per_mtok_output: o,
        })
    };
    if m.contains("gpt-4o") || m.contains("chatgpt") {
        p(2.50, 10.00)
    } else if m.contains("gpt-4-turbo") || m.contains("gpt-4.1") {
        p(10.00, 30.00)
    } else if m.contains("o1") || m.contains("o3") || m.contains("o4-") {
        p(15.00, 60.00)
    } else if m.contains("claude-3-opus") || m.contains("opus") {
        p(15.00, 75.00)
    } else if m.contains("claude-3-5-sonnet")
        || m.contains("claude-3.5-sonnet")
        || m.contains("claude-sonnet-4")
        || m.contains("sonnet")
    {
        p(3.00, 15.00)
    } else if m.contains("haiku") {
        p(0.80, 4.00)
    } else if m.contains("gemini-1.5-pro") || m.contains("gemini-2") && m.contains("pro") {
        p(1.25, 5.00)
    } else if m.contains("deepseek") {
        p(0.27, 1.10)
    } else if m.contains("llama-3") || m.contains("llama3") {
        p(0.60, 0.70)
    } else if m.contains("mistral") || m.contains("mixtral") {
        p(0.50, 1.50)
    } else {
        None
    }
}

pub fn cost_usd(
    pricing: Option<Pricing>,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> Option<f64> {
    let pr = pricing?;
    Some(
        prompt_tokens as f64 / 1_000_000.0 * pr.usd_per_mtok_input
            + completion_tokens as f64 / 1_000_000.0 * pr.usd_per_mtok_output,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_families_match() {
        assert!(for_model("anthropic/claude-sonnet-4").is_some());
        assert!(for_model("openai/gpt-4o-mini").is_some());
        assert!(for_model("deepseek/deepseek-chat").is_some());
        assert!(for_model("totally-unknown/model").is_none());
    }

    #[test]
    fn sonnet_math() {
        let pr = for_model("claude-sonnet-4").unwrap();
        let c = cost_usd(Some(pr), 1_000_000, 100_000).unwrap();
        assert!((c - (3.0 + 1.5)).abs() < 1e-9);
    }

    #[test]
    fn unknown_is_none() {
        assert!(cost_usd(None, 1000, 1000).is_none());
    }
}
