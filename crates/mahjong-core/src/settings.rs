use serde::{Deserialize, Serialize};

/// Display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    En,
    Ja,
}

/// Score threshold that ends a game early.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BankruptcyRule {
    /// Continue even when a player has no points left.
    None,
    /// End when a player's score becomes negative; exactly zero continues.
    #[default]
    Negative,
    /// End when a player's score becomes zero or negative.
    ZeroOrLess,
}

/// Whether the final dealer may stop the game while leading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AllLastRule {
    /// The dealer must rotate before the game can end.
    #[default]
    Continue,
    /// End after the final dealer wins while in first place.
    Win,
    /// End after the final dealer wins or declares tenpai while in first place.
    WinOrTenpai,
}

/// Rule settings.
///
/// Sent over the network (`CreateRoom` / `RoomState`), so the struct-level
/// `#[serde(default)]` fills missing fields with defaults. This lets old
/// clients' JSON remain parseable when new rule flags are added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Display language (default: Japanese)
    pub display_lang: Lang,
    /// Whether All Inside (Tan'yao) is allowed on an open hand
    /// (kuitan / 喰いタン; default: allowed)
    pub opened_all_inside: bool,
    /// Red fives (aka dora / 赤ドラ; default: on).
    /// Four-player games use one red five per suit; three-player games use
    /// one red 5p and 5s.
    pub red_fives: bool,
    /// Whether a tenpai dealer continues after an exhaustive draw
    /// (default: on). A dealer win always continues.
    pub tenpai_renchan: bool,
    /// Extend an East-only game into South, or a hanchan into West, when no
    /// player has reached the target score (default: off). At most one extra
    /// round wind is played.
    pub round_extension: bool,
    /// Final-dealer stopping rule (default: continue).
    pub all_last_rule: AllLastRule,
    /// Bankruptcy threshold (default: negative score).
    pub bankruptcy_rule: BankruptcyRule,
    /// End a four-player game when any player reaches 55,000 points
    /// (default: off).
    pub cold_end: bool,
    /// Four-quads abortive draw (sūkan sanra / 四槓散了; default: on).
    /// On: the hand is drawn when two or more players declare four quads in total.
    /// Off: no abortive draw, but no further quads are allowed after the fourth.
    pub four_kans_draw: bool,
    /// Four-winds abortive draw (sūfon renda / 四風連打; default: on).
    /// On: the hand is drawn when all players discard the same wind on their
    /// first discard.
    pub four_winds_draw: bool,
    /// Four-riichi abortive draw (sūcha riichi / 四家立直; default: off).
    /// On: the hand is drawn when all players declare riichi.
    pub four_riichi_draw: bool,
    /// Nine terminals abortive draw (kyūshu kyūhai / 九種九牌; default: on).
    /// On: a player whose starting hand has 9+ kinds of terminals/honours
    /// may declare an abortive draw.
    pub nine_terminals_draw: bool,
    /// Triple-ron abortive draw (sanchahō / 三家和; default: off).
    /// On: the hand is drawn when all three other players declare ron
    /// on the same discard.
    pub triple_ron_draw: bool,
    /// Whether multiple simultaneous ron wins are allowed (default: on).
    /// On: when two or three players declare ron, all of them win.
    /// Off: only the player closest in turn order to the discarder wins.
    /// When triple_ron_draw is on and three players ron, the abortive draw
    /// takes precedence over this flag.
    pub multiple_ron: bool,
    /// Forbid swap-calling (kuikae / 喰い替え; default: forbidden).
    /// On: immediately after a chii/pon, the caller may not discard the same
    /// tile kind as the called tile, nor the tile at the opposite end of a
    /// chii sequence.
    pub forbid_swap_calling: bool,
    /// Three-player mahjong (sanma / 三麻; default: false = four-player).
    /// On: 108 tiles with characters 2m-8m removed and no chii.
    pub three_player: bool,
    /// Tsumo loss (tsumo-zon / ツモ損; three-player only, default: on).
    /// On: per-person payments are unchanged and the absent North player's
    /// share is not received. Off: that share is split between both payers.
    pub tsumo_loss: bool,
    /// Pei dora (North extraction / 北抜き; three-player only, default: on).
    /// On: a player may set aside a North tile as one dora each and draw a
    /// replacement tile from the wall.
    pub nuki_dora: bool,
    /// Yakuman liability payment (pao / 包; default: on).
    /// On: the player whose discard completes the final meld of Big Dragons /
    /// Big Winds / Four Quads pays the full value on tsumo, or splits it with
    /// the deal-in player on ron.
    pub yakuman_pao: bool,
    /// Double yakuman variants (default: on).
    /// On: Four Concealed Triplets on a pair wait, Big Winds, Thirteen Orphans
    /// on a 13-sided wait, and Pure Nine Gates are worth two yakuman.
    pub double_yakuman: bool,
    /// Nagashi Mangan (流し満貫; default: on).
    pub nagashi_mangan: bool,
    /// Mangan rounding for 3 han 60 fu and 4 han 30 fu (default: on).
    pub kiriage_mangan: bool,
    /// Whether a non-yakuman hand with 13+ han is scored as counted yakuman
    /// instead of sanbaiman (default: on).
    pub counted_yakuman: bool,
    /// Whether Four Quads is included when yakuman liability payment is on
    /// (default: on). Big Dragons and Big Winds follow `yakuman_pao`.
    pub four_quads_pao: bool,
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
            red_fives: true,
            tenpai_renchan: true,
            round_extension: false,
            all_last_rule: AllLastRule::Continue,
            bankruptcy_rule: BankruptcyRule::Negative,
            cold_end: false,
            four_kans_draw: true,
            four_winds_draw: true,
            four_riichi_draw: false,
            nine_terminals_draw: true,
            triple_ron_draw: false,
            multiple_ron: true,
            forbid_swap_calling: true,
            three_player: false,
            tsumo_loss: true,
            nuki_dora: true,
            yakuman_pao: true,
            double_yakuman: true,
            nagashi_mangan: true,
            kiriage_mangan: true,
            counted_yakuman: true,
            four_quads_pao: true,
        }
    }

    /// Returns the number of players (3 for three-player games, otherwise 4).
    pub fn player_count(&self) -> usize {
        if self.three_player { 3 } else { 4 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_four_player() {
        let settings = Settings::new();
        assert!(!settings.three_player);
        assert!(settings.tsumo_loss);
        assert!(settings.nuki_dora);
        assert!(settings.double_yakuman);
        assert!(settings.red_fives);
        assert!(settings.tenpai_renchan);
        assert!(!settings.round_extension);
        assert_eq!(settings.all_last_rule, AllLastRule::Continue);
        assert_eq!(settings.bankruptcy_rule, BankruptcyRule::Negative);
        assert!(!settings.cold_end);
        assert!(settings.nagashi_mangan);
        assert!(settings.kiriage_mangan);
        assert!(settings.counted_yakuman);
        assert!(settings.four_quads_pao);
        assert_eq!(settings.player_count(), 4);
    }

    #[test]
    fn three_player_count() {
        let settings = Settings {
            three_player: true,
            ..Settings::new()
        };
        assert_eq!(settings.player_count(), 3);
    }

    /// JSON from clients that predate newer rule fields must still parse.
    #[test]
    fn deserialize_without_new_rule_fields() {
        let json = serde_json::to_string(&Settings::new()).unwrap();
        // Simulate an old format by removing fields added after the initial rules.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("three_player");
        value.as_object_mut().unwrap().remove("tsumo_loss");
        value.as_object_mut().unwrap().remove("nuki_dora");
        value.as_object_mut().unwrap().remove("double_yakuman");
        value.as_object_mut().unwrap().remove("red_fives");
        value.as_object_mut().unwrap().remove("tenpai_renchan");
        value.as_object_mut().unwrap().remove("round_extension");
        value.as_object_mut().unwrap().remove("all_last_rule");
        value.as_object_mut().unwrap().remove("bankruptcy_rule");
        value.as_object_mut().unwrap().remove("cold_end");
        value.as_object_mut().unwrap().remove("nagashi_mangan");
        value.as_object_mut().unwrap().remove("kiriage_mangan");
        value.as_object_mut().unwrap().remove("counted_yakuman");
        value.as_object_mut().unwrap().remove("four_quads_pao");
        let settings: Settings = serde_json::from_value(value).unwrap();
        assert!(!settings.three_player);
        assert!(settings.tsumo_loss);
        assert!(settings.nuki_dora);
        assert!(settings.double_yakuman);
        assert!(settings.red_fives);
        assert!(settings.tenpai_renchan);
        assert!(!settings.round_extension);
        assert_eq!(settings.all_last_rule, AllLastRule::Continue);
        assert_eq!(settings.bankruptcy_rule, BankruptcyRule::Negative);
        assert!(!settings.cold_end);
        assert!(settings.nagashi_mangan);
        assert!(settings.kiriage_mangan);
        assert!(settings.counted_yakuman);
        assert!(settings.four_quads_pao);
    }

    /// The struct-level serde default must restore every field from empty JSON,
    /// so future rule flags stay compatible with old clients' messages.
    #[test]
    fn deserialize_from_empty_object_uses_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::new());
    }
}
