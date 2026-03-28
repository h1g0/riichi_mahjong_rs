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
    /// 四槓散了ありかなしか（デフォルトはあり）
    /// ありの場合: 2人以上で合計4回カンしたら流局
    /// なしの場合: 流局にはならないが、場全体で4回カン後は追加のカン不可
    pub suukantsanra: bool,
}

impl Settings {
    pub fn new() -> Settings {
        Settings {
            display_lang: Lang::Ja,
            opened_all_simples: true,
            suukantsanra: true,
        }
    }
}
