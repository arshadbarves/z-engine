//! JSON fragments shared by the wire adapters.

use serde_json::{Value, json};

/// The prompt-cache breakpoint marker (`cache_control`).
pub(crate) fn ephemeral() -> Value {
    json!({"type": "ephemeral"})
}

/// An `f32` as the JSON number it prints as (0.2, not 0.20000000298...).
pub(crate) fn float(value: f32) -> Value {
    let shortest = value.to_string().parse::<f64>().unwrap_or(f64::from(value));
    json!(shortest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_keep_their_printed_value() {
        assert_eq!(float(0.2).to_string(), "0.2");
        assert_eq!(float(1.0).to_string(), "1.0");
        assert_eq!(ephemeral(), json!({"type": "ephemeral"}));
    }
}
