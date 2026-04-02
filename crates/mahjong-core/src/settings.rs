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
    pub four_kans_draw: bool,
    /// 四風連打ありかなしか（デフォルトはあり）
    /// ありの場合: 第一打で全員が同じ風牌を捨てたら流局
    pub four_winds_draw: bool,
    /// 四家立直ありかなしか（デフォルトはなし）
    /// ありの場合: 全員がリーチ宣言したら流局
    pub four_riichi_draw: bool,
    /// 九種九牌ありかなしか（デフォルトはあり）
    /// ありの場合: 配牌時にヤオ九牌が9種以上あれば流局宣言可能
    pub nine_terminals_draw: bool,
}

impl Settings {
    pub fn new() -> Settings {
        Settings {
            display_lang: Lang::Ja,
            opened_all_simples: true,
            four_kans_draw: true,
            four_winds_draw: true,
            four_riichi_draw: false,
            nine_terminals_draw: true,
        }
    }
}
