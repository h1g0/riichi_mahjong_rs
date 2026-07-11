use serde::{Deserialize, Serialize};

/// 表示をどの言語にするかの列挙型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    /// 英語
    En,
    /// 日本語
    Ja,
}

/// 設定
///
/// ネットワーク越しに送受信されるため（`CreateRoom` / `RoomState`）、
/// 構造体レベルの `#[serde(default)]` で欠けたフィールドを既定値に補完する。
/// これにより将来ルールフラグを追加しても旧クライアントの JSON を解釈できる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 表示言語（デフォルトは日本語）
    pub display_lang: Lang,
    /// 喰いタンありかなしか（デフォルトはあり）
    pub opened_all_inside: bool,
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
    /// 三家和流局ありかなしか（デフォルトはなし）
    /// ありの場合: 1人の捨て牌に対して3人全員がロン宣言したら流局
    pub triple_ron_draw: bool,
    /// 複数同時ロン（ダブロン・トリロン）を許可するか（デフォルトはあり）
    /// ありの場合: 2人または3人がロン宣言した場合、全員の和了を認める
    /// なしの場合: 打順が最も早い1人のみ和了を認める（上家取り）
    /// ※ triple_ron_draw=true かつ 3人ロンの場合は、こちらより三家和流局が優先される
    pub multiple_ron: bool,
    /// 喰い替え禁止ありかなしか（デフォルトはあり＝禁止する）
    /// ありの場合: チー・ポン直後の打牌で、鳴いた牌と同種（現物喰い替え）や
    /// チーで作った順子の反対端の牌（スジ喰い替え）を捨てられない
    pub forbid_swap_calling: bool,
    /// 三人麻雀（サンマ）かどうか（デフォルトは false = 四人麻雀）
    /// ありの場合: 萬子2〜8を除外した108枚で3人で打つ。チーは提供されない。
    /// ツモはツモ損（1人あたりの支払額は四麻と同じで、いない北家分は貰えない）。
    pub three_player: bool,
    /// 北抜きドラありかなしか（三人麻雀のみ有効。デフォルトはあり）
    /// ありの場合: 手番中に北風牌を晒して1枚につきドラ1として扱い、牌山から補充する。
    pub nuki_dora: bool,
    /// 役満の包（責任払い）ありかなしか（デフォルトはあり）
    /// ありの場合: 大三元・大四喜・四槓子を確定させる鳴きをさせたプレイヤーが、
    /// ツモ和了なら全額、他家への放銃なら放銃者と折半で支払う。
    pub yakuman_pao: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub fn new() -> Settings {
        Settings {
            display_lang: Lang::Ja,
            opened_all_inside: true,
            four_kans_draw: true,
            four_winds_draw: true,
            four_riichi_draw: false,
            nine_terminals_draw: true,
            triple_ron_draw: false,
            multiple_ron: true,
            forbid_swap_calling: true,
            three_player: false,
            nuki_dora: true,
            yakuman_pao: true,
        }
    }

    /// プレイヤー人数を返す（三麻なら3、四麻なら4）
    pub fn player_count(&self) -> usize {
        if self.three_player { 3 } else { 4 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// デフォルトは四麻・北抜きあり
    #[test]
    fn default_is_four_player() {
        let settings = Settings::new();
        assert!(!settings.three_player);
        assert!(settings.nuki_dora);
        assert_eq!(settings.player_count(), 4);
    }

    /// 三麻フラグでプレイヤー人数が3になる
    #[test]
    fn three_player_count() {
        let settings = Settings {
            three_player: true,
            ..Settings::new()
        };
        assert_eq!(settings.player_count(), 3);
    }

    /// 旧形式（三麻フィールドなし）の JSON からデシリアライズできる
    #[test]
    fn deserialize_without_sanma_fields() {
        let json = serde_json::to_string(&Settings::new()).unwrap();
        // three_player / nuki_dora を取り除いた旧形式を模擬
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("three_player");
        value.as_object_mut().unwrap().remove("nuki_dora");
        let settings: Settings = serde_json::from_value(value).unwrap();
        assert!(!settings.three_player);
        assert!(settings.nuki_dora);
    }

    /// 構造体レベルの serde デフォルトにより、空の JSON からでも既定値で復元できる
    /// （将来ルールフラグを追加しても旧クライアントのメッセージを解釈できる）
    #[test]
    fn deserialize_from_empty_object_uses_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::new());
    }
}
