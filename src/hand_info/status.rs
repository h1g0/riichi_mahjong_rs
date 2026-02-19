use crate::tile::Wind;

/// 手牌の（牌以外の）状態
pub struct Status {
    /// 立直したか
    pub has_claimed_ready: bool,
    /// 鳴いたか
    pub has_claimed_open: bool,
    /// 自摸しているか
    pub is_self_picked: bool,
    /// 一発が有効な間立てるフラグ
    pub is_one_shot: bool,
    /// 自風
    pub player_wind: Wind,
    /// 場風
    pub prevailing_wind: Wind,
    /// 海底（最後のツモ牌）か
    pub is_last_tile_from_the_wall: bool,
    /// 河底（最後の捨て牌）か
    pub is_last_discard: bool,
    /// 嶺上開花か
    pub is_dead_wall_draw: bool,
    /// 搶槓か
    pub is_robbing_a_quad: bool,
    /// ダブル立直か
    pub is_double_ready: bool,
    /// 親（東家）か
    pub is_dealer: bool,
    /// 第一ツモか（天和・地和の判定用）
    pub is_first_turn: bool,
    /// 流し満貫か
    pub is_nagashi_mangan: bool,
    /// 槓子の数
    pub kan_count: u32,
}

impl Status {
    pub fn new() -> Status {
        Status {
            has_claimed_ready: false,
            has_claimed_open: false,
            is_self_picked: false,
            is_one_shot: false,
            player_wind: Wind::East,
            prevailing_wind: Wind::East,
            is_last_tile_from_the_wall: false,
            is_last_discard: false,
            is_dead_wall_draw: false,
            is_robbing_a_quad: false,
            is_double_ready: false,
            is_dealer: false,
            is_first_turn: false,
            is_nagashi_mangan: false,
            kan_count: 0,
        }
    }
}
