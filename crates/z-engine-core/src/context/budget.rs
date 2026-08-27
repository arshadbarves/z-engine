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
    /// ≥ compact_at_percent (default 92): auto-compaction triggers.
    Compact,
}

#[derive(Debug, Clone, Copy)]
pub struct BudgetMeter {
    pub max_tokens: u32,
    /// Auto-compaction trigger point, as a percent of the budget
    /// (default 92). Configurable via `compact_at_percent`.
    pub compact_at_percent: u8,
}

impl BudgetMeter {
    pub fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            compact_at_percent: 92,
        }
    }

    /// Clamp to a sane band: below Warn (80) would compact constantly.
    pub fn with_compact_percent(mut self, percent: u8) -> Self {
        self.compact_at_percent = percent.clamp(80, 99);
        self
    }

    pub fn level(&self, tokens_used: u64) -> Pressure {
        let max = self.max_tokens.max(1) as f64;
        let ratio = tokens_used as f64 / max;
        if ratio >= f64::from(self.compact_at_percent) / 100.0 {
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

    #[test]
    fn compact_percent_is_configurable_and_clamped() {
        let m = BudgetMeter::new(1000).with_compact_percent(85);
        assert_eq!(m.level(849), Pressure::Warn);
        assert_eq!(m.level(850), Pressure::Compact);
        // Below the Warn band would thrash; clamped to 80.
        let low = BudgetMeter::new(1000).with_compact_percent(10);
        assert_eq!(low.compact_at_percent, 80);
    }
}
