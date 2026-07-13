use serde::{Deserialize, Serialize};

use crate::tile::Wind;

/// Hand state other than the tiles themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    /// Has declared riichi
    pub has_claimed_riichi: bool,
    /// Has called a meld (open hand)
    pub has_claimed_open: bool,
    /// Won by self-draw (tsumo)
    pub is_self_drawn: bool,
    /// Set while Unbroken (ippatsu / 一発) is still possible
    pub is_unbroken: bool,
    /// Seat wind (jikaze / 自風)
    pub seat_wind: Wind,
    /// Round wind (bakaze / 場風)
    pub round_wind: Wind,
    /// Won on the last live-wall draw (haitei / 海底)
    pub is_last_tile_draw: bool,
    /// Won on the final discard (hōtei / 河底)
    pub is_last_tile_claim: bool,
    /// Won on the replacement tile after a quad (rinshan / 嶺上開花)
    pub is_after_a_quad: bool,
    /// Won by robbing a quad (chankan / 搶槓)
    pub is_robbing_a_quad: bool,
    /// Declared riichi on the first discard (double riichi)
    pub is_double_riichi: bool,
    /// Is the dealer (East player)
    pub is_dealer: bool,
    /// First draw of the hand, for Blessing of Heaven/Earth (天和・地和)
    pub is_first_turn: bool,
    /// Qualifies for Nagashi Mangan (流し満貫)
    pub is_nagashi_mangan: bool,
    /// Number of quads declared
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
