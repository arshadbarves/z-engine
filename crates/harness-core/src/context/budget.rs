//! Token budget metering (spec §6): provider-reported usage is the
//! authoritative signal; a chars÷4 estimator covers pre-response checks.
//! (Estimator calibration is tracked in docs/deviations.md for v1.0.)

/// Rough token estimate for local text — good enough for pressure hints.
pub fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.div_ceil(4)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pressure {
    /// < 80% of budget.
    Ok,
    /// ≥ 80%: status-bar warning.
    Warn,
    /// ≥ 92%: auto-compaction triggers.
    Compact,
}

#[derive(Debug, Clone, Copy)]
pub struct BudgetMeter {
    pub max_tokens: u32,
}

impl BudgetMeter {
    pub fn new(max_tokens: u32) -> Self {
        Self { max_tokens }
    }

    pub fn level(&self, tokens_used: u64) -> Pressure {
        let max = self.max_tokens.max(1) as f64;
        let ratio = tokens_used as f64 / max;
        if ratio >= 0.92 {
            Pressure::Compact
        } else if ratio >= 0.80 {
            Pressure::Warn
        } else {
            Pressure::Ok
        }
    }

    pub fn used(&self, prompt_tokens: u64, completion_tokens: u64) -> u64 {
        // Prompt dominates the next request's cost; completion counts toward
        // the session total for display purposes.
        prompt_tokens + completion_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_is_chars_over_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2); // ceil
        assert_eq!(estimate_tokens("é".repeat(8).as_str()), 2);
    }

    #[test]
    fn pressure_levels() {
        let m = BudgetMeter::new(1000);
        assert_eq!(m.level(799), Pressure::Ok);
        assert_eq!(m.level(800), Pressure::Warn);
        assert_eq!(m.level(919), Pressure::Warn);
        assert_eq!(m.level(920), Pressure::Compact);
        assert_eq!(m.level(5000), Pressure::Compact);
    }

    #[test]
    fn zero_budget_never_panics() {
        let m = BudgetMeter::new(0);
        assert_eq!(m.level(1), Pressure::Compact);
    }
}
