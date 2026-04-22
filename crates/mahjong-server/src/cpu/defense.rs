//! 守備ロジック
//!
//! 牌の安全度を評価する。現物・筋・壁・字牌・端牌の判定。

use mahjong_core::tile::{Tile, TileType};

use super::state::CpuGameState;

/// 牌の安全度を評価する（0.0=最危険, 1.0=最安全）
///
/// リーチ者や危険なプレイヤー全員に対する安全度を総合的に評価する。
pub fn evaluate_safety(tile: Tile, state: &CpuGameState) -> f64 {
    let my_idx = match state.my_seat_wind {
        mahjong_core::tile::Wind::East => 0,
        mahjong_core::tile::Wind::South => 1,
        mahjong_core::tile::Wind::West => 2,
        mahjong_core::tile::Wind::North => 3,
    };

    let mut min_safety = 1.0f64;

    for i in 0..4 {
        if i == my_idx {
            continue;
        }

        // リーチしているプレイヤーに対してのみ安全度を計算
        // （リーチしていないプレイヤーに対しては安全とみなす）
        if !state.player_riichi[i] {
            continue;
        }

        let safety = evaluate_safety_against_player(tile, &state.all_discards[i], state);
        min_safety = min_safety.min(safety);
    }

    min_safety
}

/// 特定のプレイヤーに対する牌の安全度を評価する
fn evaluate_safety_against_player(
    tile: Tile,
    opponent_discards: &[Tile],
    state: &CpuGameState,
) -> f64 {
    let tt = tile.get();

    // 1. 現物: 相手の捨て牌に同じ牌がある → 完全安全
    if opponent_discards.iter().any(|d| d.get() == tt) {
        return 1.0;
    }

    // 2. 字牌の安全度
    if tt >= 27 {
        let visible = state.visible_tile_counts()[tt as usize];
        return match visible {
            4 => 1.0,  // 全部見えている（ありえないが念のため）
            3 => 0.95, // 残り1枚 → ほぼ安全
            2 => 0.6,  // 残り2枚
            1 => 0.4,  // 残り3枚
            _ => 0.3,  // 1枚も見えていない
        };
    }

    // 3. 筋（suji）判定
    if is_suji(tt, opponent_discards) {
        return 0.75;
    }

    // 4. 壁（kabe）判定
    let visible_counts = state.visible_tile_counts();
    if is_kabe(tt, &visible_counts) {
        return 0.7;
    }

    // 5. 端牌 vs 中張牌
    let num = tt % 9;
    match num {
        0 | 8 => 0.4, // 1, 9
        1 | 7 => 0.3, // 2, 8
        2 | 6 => 0.2, // 3, 7
        _ => 0.15,    // 4, 5, 6
    }
}

/// 筋（suji）で安全かどうか判定する
///
/// 例: 相手が4mを捨てている → 1m, 7m は筋で比較的安全
///     相手が5mを捨てている → 2m, 8m は筋
///     相手が6mを捨てている → 3m, 9m は筋
fn is_suji(tile_type: TileType, opponent_discards: &[Tile]) -> bool {
    if tile_type >= 27 {
        return false; // 字牌に筋はない
    }

    let suit_start = (tile_type / 9) * 9;
    let num = tile_type - suit_start; // 0-8

    // 筋のペア: (1,4), (2,5), (3,6), (4,7), (5,8), (6,9)
    // numは0-indexed: (0,3), (1,4), (2,5), (3,6), (4,7), (5,8)
    let suji_partner = match num {
        0 => Some(suit_start + 3), // 1 → 4
        1 => Some(suit_start + 4), // 2 → 5
        2 => Some(suit_start + 5), // 3 → 6
        3 => {
            // 4 → 1 or 7
            if opponent_discards.iter().any(|d| d.get() == suit_start)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 6)
            {
                return true;
            }
            return false;
        }
        4 => {
            // 5 → 2 or 8
            if opponent_discards.iter().any(|d| d.get() == suit_start + 1)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 7)
            {
                return true;
            }
            return false;
        }
        5 => {
            // 6 → 3 or 9
            if opponent_discards.iter().any(|d| d.get() == suit_start + 2)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 8)
            {
                return true;
            }
            return false;
        }
        6 => Some(suit_start + 3), // 7 → 4
        7 => Some(suit_start + 4), // 8 → 5
        8 => Some(suit_start + 5), // 9 → 6
        _ => None,
    };

    if let Some(partner) = suji_partner {
        opponent_discards.iter().any(|d| d.get() == partner)
    } else {
        false
    }
}

/// 壁（kabe）で安全かどうか判定する
///
/// ある牌種が場に全て見えている（残り0枚）場合、
/// その牌を含む順子が成立しないため、隣接牌の危険度が下がる。
fn is_kabe(tile_type: TileType, visible_counts: &[u8; 34]) -> bool {
    if tile_type >= 27 {
        return false; // 字牌に壁はない
    }

    let suit_start = (tile_type / 9) * 9;
    let num = tile_type - suit_start; // 0-8

    // この牌を含みうる順子の構成牌を確認
    // 例: 5m(num=4) → 345m, 456m, 567m の構成牌 3,4,6,7 のいずれかが壁なら安全寄り
    let mut blocked_patterns = 0;
    let total_patterns;

    match num {
        0 => {
            // 1: 123 のみ。2か3が壁なら安全
            total_patterns = 1;
            if visible_counts[(suit_start + 1) as usize] >= 4
                || visible_counts[(suit_start + 2) as usize] >= 4
            {
                blocked_patterns = 1;
            }
        }
        1 => {
            // 2: 123, 234。
            total_patterns = 2;
            if visible_counts[suit_start as usize] >= 4
                || visible_counts[(suit_start + 2) as usize] >= 4
            {
                blocked_patterns += 1;
            }
            if visible_counts[(suit_start + 2) as usize] >= 4
                || visible_counts[(suit_start + 3) as usize] >= 4
            {
                blocked_patterns += 1;
            }
        }
        7 => {
            // 8: 789, 678
            total_patterns = 2;
            if visible_counts[(suit_start + 8) as usize] >= 4
                || visible_counts[(suit_start + 6) as usize] >= 4
            {
                blocked_patterns += 1;
            }
            if visible_counts[(suit_start + 6) as usize] >= 4
                || visible_counts[(suit_start + 5) as usize] >= 4
            {
                blocked_patterns += 1;
            }
        }
        8 => {
            // 9: 789 のみ。7か8が壁なら安全
            total_patterns = 1;
            if visible_counts[(suit_start + 6) as usize] >= 4
                || visible_counts[(suit_start + 7) as usize] >= 4
            {
                blocked_patterns = 1;
            }
        }
        _ => {
            // 3-7: 3パターン
            total_patterns = 3;
            // 前方の順子
            if num >= 2
                && (visible_counts[(suit_start + num - 2) as usize] >= 4
                    || visible_counts[(suit_start + num - 1) as usize] >= 4)
            {
                blocked_patterns += 1;
            }
            // 中央の順子
            if (1..=7).contains(&num)
                && (visible_counts[(suit_start + num - 1) as usize] >= 4
                    || visible_counts[(suit_start + num + 1) as usize] >= 4)
            {
                blocked_patterns += 1;
            }
            // 後方の順子
            if num <= 6
                && (visible_counts[(suit_start + num + 1) as usize] >= 4
                    || visible_counts[(suit_start + num + 2) as usize] >= 4)
            {
                blocked_patterns += 1;
            }
        }
    }

    // 全パターンが壁でブロックされていれば安全
    blocked_patterns > 0 && blocked_patterns >= total_patterns
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::{Tile, Wind};

    #[test]
    fn test_genbutsu() {
        let discards = vec![Tile::new(Tile::M5)];
        let state = CpuGameState::new();
        let safety = evaluate_safety_against_player(Tile::new(Tile::M5), &discards, &state);
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_suji_basic() {
        // 4mが捨てられている → 1m は筋で安全
        let discards = vec![Tile::new(Tile::M4)];
        assert!(is_suji(Tile::M1, &discards));
        assert!(is_suji(Tile::M7, &discards));
        assert!(!is_suji(Tile::M5, &discards));
    }

    #[test]
    fn test_suji_middle() {
        // 5mが捨てられている → 2m, 8m は筋
        let discards = vec![Tile::new(Tile::M5)];
        assert!(is_suji(Tile::M2, &discards));
        assert!(is_suji(Tile::M8, &discards));
    }

    #[test]
    fn test_honor_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = Vec::new();
        // 字牌で見えていない → 低い安全度
        let safety = evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state);
        assert!(safety < 0.5);
    }

    // --- evaluate_safety (public) ---

    #[test]
    fn test_evaluate_safety_no_riichi_returns_1() {
        // リーチ者がいなければ常に安全度 1.0
        let state = CpuGameState::new();
        let safety = evaluate_safety(Tile::new(Tile::M5), &state);
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_skips_self() {
        // 自分自身のリーチはスキップされる
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[0] = true; // 東（自分）がリーチ
        let safety = evaluate_safety(Tile::new(Tile::M5), &state);
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_genbutsu_riichi_opponent() {
        // リーチ者の現物は安全度 1.0
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true; // 南がリーチ
        state.all_discards[1] = vec![Tile::new(Tile::M5)];
        let safety = evaluate_safety(Tile::new(Tile::M5), &state);
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_multiple_riichi_takes_min() {
        // 複数リーチ者がいる場合、最小の安全度を返す
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true; // 南がリーチ: M5は現物 → 1.0
        state.player_riichi[2] = true; // 西がリーチ: M5は非現物 → 低い安全度
        state.all_discards[1] = vec![Tile::new(Tile::M5)];
        let safety = evaluate_safety(Tile::new(Tile::M5), &state);
        assert!(safety < 1.0);
    }

    // --- is_suji: 未カバーの num パターン ---

    #[test]
    fn test_suji_3m_when_6m_discarded() {
        // 6m捨て → 3m, 9m は筋
        let discards = vec![Tile::new(Tile::M6)];
        assert!(is_suji(Tile::M3, &discards));
        assert!(is_suji(Tile::M9, &discards));
    }

    #[test]
    fn test_suji_4m_when_1m_or_7m_discarded() {
        // 1m または 7m が捨てられている → 4m は筋
        assert!(is_suji(Tile::M4, &[Tile::new(Tile::M1)]));
        assert!(is_suji(Tile::M4, &[Tile::new(Tile::M7)]));
        assert!(!is_suji(Tile::M4, &[Tile::new(Tile::M2)]));
    }

    #[test]
    fn test_suji_5m_when_2m_or_8m_discarded() {
        // 2m または 8m が捨てられている → 5m は筋
        assert!(is_suji(Tile::M5, &[Tile::new(Tile::M2)]));
        assert!(is_suji(Tile::M5, &[Tile::new(Tile::M8)]));
        assert!(!is_suji(Tile::M5, &[Tile::new(Tile::M1)]));
    }

    #[test]
    fn test_suji_6m_when_3m_or_9m_discarded() {
        // 3m または 9m が捨てられている → 6m は筋
        assert!(is_suji(Tile::M6, &[Tile::new(Tile::M3)]));
        assert!(is_suji(Tile::M6, &[Tile::new(Tile::M9)]));
        assert!(!is_suji(Tile::M6, &[Tile::new(Tile::M2)]));
    }

    #[test]
    fn test_suji_pin_suit() {
        // 筋は別の色でも成立し、異なる色の捨て牌は無効
        let discards = vec![Tile::new(Tile::P4)];
        assert!(is_suji(Tile::P1, &discards));
        assert!(is_suji(Tile::P7, &discards));
        assert!(!is_suji(Tile::M1, &discards)); // 万子には無関係
    }

    #[test]
    fn test_suji_sou_suit() {
        // 索子でも同様
        let discards = vec![Tile::new(Tile::S5)];
        assert!(is_suji(Tile::S2, &discards));
        assert!(is_suji(Tile::S8, &discards));
    }

    #[test]
    fn test_suji_honor_tile_returns_false() {
        // 字牌に筋はない
        let discards = vec![Tile::new(Tile::M4)];
        assert!(!is_suji(Tile::Z1, &discards));
        assert!(!is_suji(Tile::Z7, &discards));
    }

    #[test]
    fn test_suji_no_partner_in_discards() {
        // 筋パートナーが捨てられていなければ false
        let discards = vec![Tile::new(Tile::M3)];
        assert!(!is_suji(Tile::M1, &discards)); // M1 のパートナーは M4
    }

    // --- evaluate_safety_against_player: 各安全度の返り値 ---

    #[test]
    fn test_suji_safety_value() {
        // 筋牌の安全度は 0.75
        let discards = vec![Tile::new(Tile::M4)];
        let state = CpuGameState::new();
        let safety = evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state);
        assert_eq!(safety, 0.75);
    }

    #[test]
    fn test_kabe_safety_value() {
        // 壁牌の安全度は 0.70
        // 2m が4枚見えている → 1m を含む唯一の順子(123m)がブロック
        let mut state = CpuGameState::new();
        state.all_discards[0] = vec![Tile::new(Tile::M2); 4];
        let discards: Vec<Tile> = vec![];
        let safety = evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state);
        assert_eq!(safety, 0.7);
    }

    #[test]
    fn test_end_tile_safety() {
        // 1m / 9m → 0.4
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state),
            0.4
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M9), &discards, &state),
            0.4
        );
    }

    #[test]
    fn test_near_end_tile_safety() {
        // 2m / 8m → 0.3
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M2), &discards, &state),
            0.3
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M8), &discards, &state),
            0.3
        );
    }

    #[test]
    fn test_3_7_tile_safety() {
        // 3m / 7m → 0.2
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M3), &discards, &state),
            0.2
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M7), &discards, &state),
            0.2
        );
    }

    #[test]
    fn test_middle_tile_safety() {
        // 4m / 5m / 6m → 0.15
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M4), &discards, &state),
            0.15
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M5), &discards, &state),
            0.15
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M6), &discards, &state),
            0.15
        );
    }

    #[test]
    fn test_honor_tile_visible_counts() {
        // 字牌の見え枚数に応じた安全度
        let discards: Vec<Tile> = vec![];
        {
            let state = CpuGameState::new();
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.3
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1)];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.4
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1); 2];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.6
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1); 3];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.95
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1); 4];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                1.0
            );
        }
    }

    // --- is_kabe ---

    #[test]
    fn test_kabe_1m_when_2m_exhausted() {
        // 2m が4枚見えている → 1m の唯一の順子(123m)がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M2 as usize] = 4;
        assert!(is_kabe(Tile::M1, &counts));
    }

    #[test]
    fn test_kabe_1m_not_blocked() {
        // 壁牌がなければ false
        let counts = [0u8; 34];
        assert!(!is_kabe(Tile::M1, &counts));
    }

    #[test]
    fn test_kabe_9m_when_8m_exhausted() {
        // 8m が4枚見えている → 9m の唯一の順子(789m)がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M8 as usize] = 4;
        assert!(is_kabe(Tile::M9, &counts));
    }

    #[test]
    fn test_kabe_2m_fully_blocked() {
        // 3m が4枚見えている → 123m と 234m の両方がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M3 as usize] = 4;
        assert!(is_kabe(Tile::M2, &counts));
    }

    #[test]
    fn test_kabe_2m_partially_blocked() {
        // 1m のみ4枚 → 123m はブロックされるが 234m はブロックされない → false
        let mut counts = [0u8; 34];
        counts[Tile::M1 as usize] = 4;
        assert!(!is_kabe(Tile::M2, &counts));
    }

    #[test]
    fn test_kabe_8m_fully_blocked() {
        // 7m が4枚見えている → 789m と 678m の両方がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M7 as usize] = 4;
        assert!(is_kabe(Tile::M8, &counts));
    }

    #[test]
    fn test_kabe_middle_5m_fully_blocked() {
        // 4m + 6m が4枚見えている → 345m / 456m / 567m 全てブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M4 as usize] = 4;
        counts[Tile::M6 as usize] = 4;
        assert!(is_kabe(Tile::M5, &counts));
    }

    #[test]
    fn test_kabe_middle_5m_partially_blocked() {
        // 4m のみ4枚 → 345m と 456m はブロックされるが 567m はブロックされない → false
        let mut counts = [0u8; 34];
        counts[Tile::M4 as usize] = 4;
        assert!(!is_kabe(Tile::M5, &counts));
    }

    #[test]
    fn test_kabe_pin_suit() {
        // 別のスートでも壁判定が成立する
        let mut counts = [0u8; 34];
        counts[Tile::P2 as usize] = 4;
        assert!(is_kabe(Tile::P1, &counts));
    }

    #[test]
    fn test_kabe_honor_tile_returns_false() {
        // 字牌に壁はない
        let mut counts = [0u8; 34];
        counts[Tile::Z1 as usize] = 4;
        assert!(!is_kabe(Tile::Z1, &counts));
    }
}
