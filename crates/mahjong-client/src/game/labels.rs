//! Player-type labels and CPU display names.

use super::*;

/// Player type per seat, used to display level and personality.
#[derive(Debug, Clone)]
pub enum PlayerLabel {
    /// This client's own seat
    Me,
    /// Another human (an online opponent)
    Human(String),
    /// A CPU with its level and personality
    Cpu { level: String, personality: String },
}

/// Localizes a CPU level's internal English name.
pub(super) fn localize_cpu_level(level: &str, lang: Lang) -> &'static str {
    let idx = match level {
        "Weak" => 0,
        "Strong" => 2,
        _ => 1,
    };
    Translator::new(lang).strength_label(idx)
}

/// Localizes a CPU personality's internal English name.
pub(super) fn localize_cpu_personality(personality: &str, lang: Lang) -> &'static str {
    let idx = match personality {
        "Speedy" => 1,
        "HighValue" => 2,
        "Defensive" => 3,
        _ => 0,
    };
    Translator::new(lang).personality_label(idx)
}

impl PlayerLabel {
    /// Caption under the wind/score display; hidden for our own seat.
    /// CPUs show "CPU{n} (level, personality)", humans their name.
    pub fn detail(&self, cpu_number: usize, lang: Lang) -> Option<String> {
        match self {
            PlayerLabel::Me => None,
            PlayerLabel::Human(name) => Some(name.clone()),
            PlayerLabel::Cpu { level, personality } => {
                Some(cpu_display(cpu_number, level, personality, lang))
            }
        }
    }

    /// Display name for rankings; CPUs show "CPU{n} (level, personality)".
    pub fn name(&self, cpu_number: usize, lang: Lang) -> String {
        match self {
            PlayerLabel::Me => Key::You.text(lang).to_string(),
            PlayerLabel::Human(name) => name.clone(),
            PlayerLabel::Cpu { level, personality } => {
                cpu_display(cpu_number, level, personality, lang)
            }
        }
    }

    /// Short display name for score chips and results (e.g. "CPU2").
    pub fn short_name(&self, rel: usize, lang: Lang) -> String {
        match self {
            PlayerLabel::Me => Key::You.text(lang).to_string(),
            PlayerLabel::Human(name) => {
                let mut s: String = name.chars().take(5).collect();
                if name.chars().count() > 5 {
                    s.push('…');
                }
                s
            }
            PlayerLabel::Cpu { .. } => format!("CPU{}", rel),
        }
    }
}

/// CPU display name, e.g. "CPU1（普通・バランス）" / "CPU1 (Normal, Balanced)".
pub(super) fn cpu_display(cpu_number: usize, level: &str, personality: &str, lang: Lang) -> String {
    let lv = localize_cpu_level(level, lang);
    let ps = localize_cpu_personality(personality, lang);
    match lang {
        Lang::Ja => format!("CPU{cpu_number}（{lv}・{ps}）"),
        Lang::En => format!("CPU{cpu_number} ({lv}, {ps})"),
    }
}

/// Builds a CPU [`PlayerLabel`] from its config.
pub(super) fn cpu_label(config: &CpuConfig) -> PlayerLabel {
    PlayerLabel::Cpu {
        level: config.level.display_name().to_string(),
        personality: config.personality.display_name().to_string(),
    }
}
