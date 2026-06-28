//! クライアントUIの多言語化（i18n）
//!
//! 表示言語は [`mahjong_core::settings::Lang`] を再利用する。固定の文言は
//! [`Key`] 列挙型にキーごとへ全言語まとめて定義し（翻訳の取りこぼし防止）、
//! 数値や牌などを含む可変の文言は [`Translator`] のメソッドで組み立てる
//! （言語ごとの語順差を吸収するため）。
//!
//! 役名・点数等級・風・ドラなどゲーム由来の用語は `mahjong-core` 側の
//! ローカライズ関数を正典とし、ここでは UI 固有の文言のみを扱う。

use mahjong_core::settings::Lang;

/// 現在の表示言語を保持し、文言を解決する軽量ハンドル。
///
/// [`GameState`](crate::game::GameState) が保持し、描画関数へ `&GameState`
/// 経由で渡る。`Copy` なので自由に複製してよい。
#[derive(Debug, Clone, Copy)]
pub struct Translator {
    lang: Lang,
}

impl Translator {
    /// 指定言語の [`Translator`] を作る。
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// 固定文言を解決する。
    pub fn get(&self, key: Key) -> &'static str {
        key.text(self.lang)
    }

    /// CPU の強さラベル（0=弱い, 1=普通, 2=強い）。
    pub fn strength_label(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["弱い", "普通", "強い"],
            Lang::En => ["Weak", "Normal", "Strong"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// CPU の性格ラベル（0=バランス, 1=スピード, 2=高得点, 3=守備的）。
    pub fn personality_label(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["バランス", "スピード", "高得点", "守備的"],
            Lang::En => ["Balanced", "Speedy", "High Value", "Defensive"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// 自分から見た相対座席名（0=下家, 1=対面, 2=上家）。
    pub fn seat_relative(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["下家", "対面", "上家"],
            Lang::En => ["Right", "Across", "Left"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }
}

/// 引数を取らない固定 UI 文言のキー。
///
/// 各バリアントの訳は [`Key::text`] にまとめて定義する。言語を増やすときは
/// `Lang` に追加し、各 `match` 腕へ訳を足す（コンパイル時に網羅性が保証される）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// 対局設定画面のタイトル
    SetupTitle,
    /// CPU の強さ見出し
    CpuStrengthLabel,
    /// CPU の性格見出し
    CpuPersonalityLabel,
    /// ローカル対局を開始するボタン
    StartGame,
    /// オンライン対戦へ進むボタン
    OnlinePlay,
}

impl Key {
    /// 指定言語での文言を返す。
    pub fn text(self, lang: Lang) -> &'static str {
        match self {
            Key::SetupTitle => match lang {
                Lang::Ja => "対局設定",
                Lang::En => "Game Setup",
            },
            Key::CpuStrengthLabel => match lang {
                Lang::Ja => "強さ",
                Lang::En => "Strength",
            },
            Key::CpuPersonalityLabel => match lang {
                Lang::Ja => "性格",
                Lang::En => "Style",
            },
            Key::StartGame => match lang {
                Lang::Ja => "対局開始",
                Lang::En => "Start Game",
            },
            Key::OnlinePlay => match lang {
                Lang::Ja => "オンライン対戦",
                Lang::En => "Online Play",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_resolves_both_languages() {
        assert_eq!(Key::StartGame.text(Lang::Ja), "対局開始");
        assert_eq!(Key::StartGame.text(Lang::En), "Start Game");
    }

    #[test]
    fn translator_indexed_labels() {
        let ja = Translator::new(Lang::Ja);
        let en = Translator::new(Lang::En);
        assert_eq!(ja.strength_label(0), "弱い");
        assert_eq!(en.strength_label(2), "Strong");
        assert_eq!(ja.personality_label(3), "守備的");
        assert_eq!(en.personality_label(0), "Balanced");
        assert_eq!(ja.seat_relative(1), "対面");
        assert_eq!(en.seat_relative(2), "Left");
    }

    #[test]
    fn out_of_range_index_is_empty() {
        let t = Translator::new(Lang::En);
        assert_eq!(t.strength_label(9), "");
    }
}
