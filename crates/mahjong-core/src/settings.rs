use serde::{Deserialize, Serialize};

/// 表示をどの言語にするかの列挙型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Lang {
    /// 英語
    En,
    /// 日本語
    Ja,
}

/// 設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// 表示言語（デフォルトは日本語）
    pub display_lang: Lang,
    /// 喰いタンありかなしか（デフォルトはあり）
    pub opened_all_simples: bool,
}

impl Settings {
    pub fn new() -> Settings {
        Settings {
            display_lang: Lang::Ja,
            opened_all_simples: true,
        }
    }
}
