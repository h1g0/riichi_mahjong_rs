use serde::{Deserialize, Serialize};

use crate::tile::Wind;

/// 手牌の（牌以外の）状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    /// 立直したか
    pub has_claimed_riichi: bool,
    /// 鳴いたか
    pub has_claimed_open: bool,
    /// 自摸しているか
    pub is_self_drawn: bool,
    /// 一発が有効な間立てるフラグ
    pub is_unbroken: bool,
    /// 自風
    pub seat_wind: Wind,
    /// 場風
    pub round_wind: Wind,
    /// 海底（最後のツモ牌）か
    pub is_last_tile_draw: bool,
    /// 河底（最後の捨て牌）か
    pub is_last_tile_claim: bool,
    /// 嶺上開花か
    pub is_after_a_quad: bool,
    /// 搶槓か
    pub is_robbing_a_quad: bool,
    /// ダブル立直か
    pub is_double_riichi: bool,
    /// 親（東家）か
    pub is_dealer: bool,
    /// 第一ツモか（天和・地和の判定用）
    pub is_first_turn: bool,
    /// 流し満貫か
    pub is_nagashi_mangan: bool,
    /// 槓子の数
    pub kan_count: u32,
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}

impl Status {
    pub fn new() -> Status {
        Status {
            has_claimed_riichi: false,
            has_claimed_open: false,
            is_self_drawn: false,
            is_unbroken: false,
            seat_wind: Wind::East,
            round_wind: Wind::East,
            is_last_tile_draw: false,
            is_last_tile_claim: false,
            is_after_a_quad: false,
            is_robbing_a_quad: false,
            is_double_riichi: false,
            is_dealer: false,
            is_first_turn: false,
            is_nagashi_mangan: false,
            kan_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_new_defaults() {
        let s = Status::new();
        assert!(!s.has_claimed_riichi);
        assert!(!s.has_claimed_open);
        assert!(!s.is_self_drawn);
        assert!(!s.is_unbroken);
        assert!(matches!(s.seat_wind, Wind::East));
        assert!(matches!(s.round_wind, Wind::East));
        assert!(!s.is_last_tile_draw);
        assert!(!s.is_last_tile_claim);
        assert!(!s.is_after_a_quad);
        assert!(!s.is_robbing_a_quad);
        assert!(!s.is_double_riichi);
        assert!(!s.is_dealer);
        assert!(!s.is_first_turn);
        assert!(!s.is_nagashi_mangan);
        assert_eq!(s.kan_count, 0);
    }
}
