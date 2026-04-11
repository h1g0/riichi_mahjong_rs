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
    use mahjong_core::tile::Tile;

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
}
