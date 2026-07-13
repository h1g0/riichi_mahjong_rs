use serde::{Deserialize, Serialize};

/// Display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    En,
    Ja,
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
    /// On: 108 tiles with characters 2m-8m removed, no chii, and tsumo loss
    /// (per-person payments are unchanged; the absent player's share is
    /// simply not received).
    pub three_player: bool,
    /// Pei dora (North extraction / 北抜き; three-player only, default: on).
    /// On: a player may set aside a North tile as one dora each and draw a
    /// replacement tile from the wall.
    pub nuki_dora: bool,
    /// Yakuman liability payment (pao / 包; default: on).
    /// On: the player whose discard completes the final meld of Big Dragons /
    /// Big Winds / Four Quads pays the full value on tsumo, or splits it with
    /// the deal-in player on ron.
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
        assert!(settings.nuki_dora);
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

    /// JSON from clients that predate the three-player fields must still parse.
    #[test]
    fn deserialize_without_sanma_fields() {
        let json = serde_json::to_string(&Settings::new()).unwrap();
        // Simulate the old format by removing three_player / nuki_dora.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("three_player");
        value.as_object_mut().unwrap().remove("nuki_dora");
        let settings: Settings = serde_json::from_value(value).unwrap();
        assert!(!settings.three_player);
        assert!(settings.nuki_dora);
    }

    /// The struct-level serde default must restore every field from empty JSON,
    /// so future rule flags stay compatible with old clients' messages.
    #[test]
    fn deserialize_from_empty_object_uses_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::new());
    }
}
