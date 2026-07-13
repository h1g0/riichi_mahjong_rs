//! Preset personality definitions: the parameter set for each
//! personality type.

use super::client::{CpuConfig, CpuLevel, CpuPersonality, PersonalityParams};

impl PersonalityParams {
    /// Builds the parameter set for a personality.
    pub fn from_personality(personality: CpuPersonality) -> Self {
        match personality {
            CpuPersonality::Balanced => PersonalityParams {
                call_aggressiveness: 0.5,
                value_weight: 0.5,
                speed_weight: 0.5,
                retreat_threshold: 0.5,
                riichi_aggressiveness: 0.6,
            },
            CpuPersonality::Speedy => PersonalityParams {
                call_aggressiveness: 0.8,
                value_weight: 0.2,
                speed_weight: 0.9,
                retreat_threshold: 0.3,
                riichi_aggressiveness: 0.4,
            },
            CpuPersonality::HighValue => PersonalityParams {
                call_aggressiveness: 0.2,
                value_weight: 0.9,
                speed_weight: 0.3,
                retreat_threshold: 0.4,
                riichi_aggressiveness: 0.9,
            },
            CpuPersonality::Defensive => PersonalityParams {
                call_aggressiveness: 0.3,
                value_weight: 0.3,
                speed_weight: 0.4,
                retreat_threshold: 0.7,
                riichi_aggressiveness: 0.5,
            },
        }
    }
}

/// Returns the preset CPU configurations.
pub fn preset_configs() -> Vec<CpuConfig> {
    vec![
        CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::Defensive),
    ]
}

/// Default configurations for the three CPU opponents:
/// a balanced mix of personalities.
pub fn default_cpu_configs() -> [CpuConfig; 3] {
    [
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::HighValue),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_personality_params() {
        let balanced = PersonalityParams::from_personality(CpuPersonality::Balanced);
        assert_eq!(balanced.call_aggressiveness, 0.5);

        let speedy = PersonalityParams::from_personality(CpuPersonality::Speedy);
        assert!(speedy.call_aggressiveness > balanced.call_aggressiveness);
        assert!(speedy.speed_weight > balanced.speed_weight);
    }

    #[test]
    fn test_preset_configs() {
        let presets = preset_configs();
        assert_eq!(presets.len(), 4);
    }

    #[test]
    fn test_default_cpu_configs() {
        let configs = default_cpu_configs();
        assert_eq!(configs.len(), 3);
    }
}
