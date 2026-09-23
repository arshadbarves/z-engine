//! Model roles and per-model limits: the `fast`/`review` roles fall back to
//! the session's main model; windows, output ceilings and prices prefer
//! settings, then the catalog, then built-in defaults.

use z_engine_config::Settings;
use z_engine_llm::{ModelCatalog, Pricing, price_for};

/// Used when neither settings nor the catalog know the window.
pub(crate) const DEFAULT_CONTEXT_WINDOW: u64 = 128_000;

pub(crate) fn fast_model(settings: &Settings, main: &str) -> String {
    settings
        .model
        .fast
        .clone()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| main.to_string())
}

pub(crate) fn context_window(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
) -> u64 {
    if let Some(window) = settings.model.context_window.filter(|w| *w > 0) {
        return u64::from(window);
    }
    catalog
        .and_then(|catalog| catalog.lookup(model))
        .map(|info| u64::from(info.context_window))
        .filter(|window| *window > 0)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW)
}

/// The configured ceiling, lowered to the model's own limit when known.
pub(crate) fn max_output_tokens(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
) -> u32 {
    let configured = settings.model.max_output_tokens;
    match catalog
        .and_then(|catalog| catalog.lookup(model))
        .map(|info| info.max_output)
    {
        Some(limit) if limit > 0 => configured.min(limit),
        _ => configured,
    }
}

/// `[pricing]` overrides win; then the catalog and the built-in table.
pub(crate) fn pricing(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
) -> Option<Pricing> {
    let bare = model.split_once('/').map_or(model, |(_, rest)| rest);
    let configured = settings.pricing.get(model).or_else(|| {
        settings
            .pricing
            .iter()
            .find(|(id, _)| id.split_once('/').map_or(id.as_str(), |(_, rest)| rest) == bare)
            .map(|(_, price)| price)
    });
    if let Some(price) = configured {
        return Some(Pricing {
            input: price.input,
            output: price.output,
            cache_read: price.cache_read.unwrap_or(price.input),
            cache_write: price.cache_write.unwrap_or(price.input),
        });
    }
    price_for(catalog, model)
}

/// Unknown models are assumed to accept images.
pub(crate) fn supports_vision(catalog: Option<&ModelCatalog>, model: &str) -> bool {
    catalog
        .and_then(|catalog| catalog.lookup(model))
        .is_none_or(|info| info.vision)
}

#[cfg(test)]
mod tests {
    use z_engine_config::PricingOverride;

    use super::*;

    #[test]
    fn roles_and_windows_fall_back_in_order() {
        let mut settings = Settings::default();
        assert_eq!(fast_model(&settings, "main-model"), "main-model");
        settings.model.fast = Some("quick".into());
        assert_eq!(fast_model(&settings, "main-model"), "quick");
        assert_eq!(context_window(&settings, None, "x"), DEFAULT_CONTEXT_WINDOW);
        settings.model.context_window = Some(9_000);
        assert_eq!(context_window(&settings, None, "x"), 9_000);
    }

    #[test]
    fn pricing_overrides_match_with_or_without_vendor() {
        let mut settings = Settings::default();
        settings.pricing.insert(
            "acme/model".into(),
            PricingOverride {
                input: 1.0,
                output: 2.0,
                cache_read: None,
                cache_write: Some(4.0),
            },
        );
        let price = pricing(&settings, None, "model").unwrap();
        assert_eq!(
            (price.input, price.cache_read, price.cache_write),
            (1.0, 1.0, 4.0)
        );
        assert!(pricing(&settings, None, "unknown-model").is_none());
    }
}
