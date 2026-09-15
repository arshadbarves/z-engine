use serde::{Deserialize, Serialize};

use crate::DiscoveryError;

/// All limits are validated, not silently clamped. Directory entries include ignored entries.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiscoveryOptions {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_profiles: usize,
    pub max_manifest_bytes: usize,
    pub max_total_bytes: usize,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_entries: 20_000,
            max_profiles: 128,
            max_manifest_bytes: 262_144,
            max_total_bytes: 4_194_304,
        }
    }
}

impl DiscoveryOptions {
    pub fn validate(&self) -> Result<(), DiscoveryError> {
        for (name, value, minimum, maximum) in [
            ("max_depth", self.max_depth, 0, 16),
            ("max_entries", self.max_entries, 1, 50_000),
            ("max_profiles", self.max_profiles, 1, 256),
            ("max_manifest_bytes", self.max_manifest_bytes, 1, 1_048_576),
            ("max_total_bytes", self.max_total_bytes, 1, 8_388_608),
        ] {
            if !(minimum..=maximum).contains(&value) {
                return Err(DiscoveryError::InvalidOptions(format!(
                    "{name} must be between {minimum} and {maximum}, got {value}"
                )));
            }
        }
        Ok(())
    }
}
