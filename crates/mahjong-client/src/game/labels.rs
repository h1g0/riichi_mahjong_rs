//! プレイヤー種別ラベルとCPU表示名

use super::*;

/// 各座席のプレイヤー種別（強さ・性格の表示に使う）
#[derive(Debug, Clone)]
pub enum PlayerLabel {
    /// 自分
    Me,
    /// 他の人間プレイヤー（オンライン対戦の相手）
    Human(String),
    /// CPU（強さ・性格つき）
    Cpu { level: String, personality: String },
}

/// CPU の強さ（英語の内部名）を表示言語へ変換する。
pub(super) fn localize_cpu_level(level: &str, lang: Lang) -> &'static str {
    let idx = match level {
        "Weak" => 0,
        "Strong" => 2,
        _ => 1,
    };
    Translator::new(lang).strength_label(idx)
}

/// CPU の性格（英語の内部名）を表示言語へ変換する。
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
    /// 風・得点の下に表示する補助テキスト（自分は非表示）。
    /// CPU は「CPU{n}（強さ・性格）」、人間プレイヤーは名前を返す。
    pub fn detail(&self, cpu_number: usize, lang: Lang) -> Option<String> {
        match self {
            PlayerLabel::Me => None,
            PlayerLabel::Human(name) => Some(name.clone()),
            PlayerLabel::Cpu { level, personality } => {
                Some(cpu_display(cpu_number, level, personality, lang))
            }
        }
    }

    /// 順位表などで使う表示名。CPU は「CPU{n}（強さ・性格）」。
    pub fn name(&self, cpu_number: usize, lang: Lang) -> String {
        match self {
            PlayerLabel::Me => Key::You.text(lang).to_string(),
            PlayerLabel::Human(name) => name.clone(),
            PlayerLabel::Cpu { level, personality } => {
                cpu_display(cpu_number, level, personality, lang)
            }
        }
    }

    /// 得点チップや和了結果などで使う短い表示名（例:「CPU2」）。
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

/// CPU の表示名（例: 日「CPU1（普通・バランス）」/ 英「CPU1 (Normal, Balanced)」）。
pub(super) fn cpu_display(cpu_number: usize, level: &str, personality: &str, lang: Lang) -> String {
    let lv = localize_cpu_level(level, lang);
    let ps = localize_cpu_personality(personality, lang);
    match lang {
        Lang::Ja => format!("CPU{cpu_number}（{lv}・{ps}）"),
        Lang::En => format!("CPU{cpu_number} ({lv}, {ps})"),
    }
}

/// CPU設定から CPU 用の [`PlayerLabel`] を作る
pub(super) fn cpu_label(config: &CpuConfig) -> PlayerLabel {
    PlayerLabel::Cpu {
        level: config.level.display_name().to_string(),
        personality: config.personality.display_name().to_string(),
    }
}
