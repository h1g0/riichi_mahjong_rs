//! 手牌評価
//!
//! 各牌を捨てた場合の向聴数・有効牌数・推定打点を計算する。
//! 入力は CpuGameState のみ（サーバ内部にはアクセスしない）。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::shanten_number;
use mahjong_core::tile::{dora_indicator_to_dora, Tile, TileType, Wind};

use super::client::CpuConfig;
use super::defense;
use super::state::CpuGameState;

/// 牌1枚を捨てた場合の評価
#[derive(Debug, Clone)]
pub struct DiscardCandidate {
    /// 捨てる牌
    pub tile: Tile,
    /// 捨てた後の向聴数
    pub shanten: i32,
    /// 有効牌の残り枚数（受入数）
    pub acceptance_count: u32,
    /// 推定打点スコア（高いほど良い）
    pub estimated_value: f64,
    /// 安全度（0.0=最危険, 1.0=最安全）
    pub safety: f64,
}

/// 手牌の全候補牌について評価を行う
///
/// hand: 手牌（13枚）+ ツモ牌 の全牌リスト（14枚）
/// opened: 副露情報を含む Hand（HandAnalyzer に渡す用）
pub fn evaluate_discards(state: &CpuGameState, config: &CpuConfig) -> Vec<DiscardCandidate> {
    let mut all_tiles = state.my_hand.clone();
    if let Some(drawn) = state.my_drawn {
        all_tiles.push(drawn);
    }

    if all_tiles.is_empty() {
        return Vec::new();
    }

    let visible_counts = state.visible_tile_counts();
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (i, &tile) in all_tiles.iter().enumerate() {
        // 同じ牌種は重複評価しない
        if !seen.insert(tile.get()) {
            continue;
        }

        // この牌を捨てた場合の残り手牌を構築
        let mut remaining: Vec<Tile> = all_tiles.clone();
        remaining.remove(i);

        // 向聴数を高速計算（Vec割り当てなし）
        let hand = Hand::new(remaining.clone(), None);
        let shanten = shanten_number(&hand);

        // 有効牌数（受入数）を計算（既知の向聴数を渡して重複計算を回避）
        let acceptance_count = if config.level.uses_acceptance_count() {
            count_acceptance(&remaining, &visible_counts, shanten)
        } else {
            0
        };

        // 推定打点
        let estimated_value = if config.level.uses_value_estimation() {
            estimate_hand_value(&remaining, state)
        } else {
            0.0
        };

        // 安全度
        let safety = if config.level.uses_defense() {
            defense::evaluate_safety(tile, state)
        } else {
            0.5 // 防御を考慮しない場合は中立値
        };

        candidates.push(DiscardCandidate {
            tile,
            shanten,
            acceptance_count,
            estimated_value,
            safety,
        });
    }

    candidates
}

/// 有効牌（受入牌）の残り枚数をカウントする
///
/// 13枚の手牌に対して、各牌種を加えた時に向聴数が下がるものをカウント。
/// `current_shanten` は呼び出し元で既に計算済みの向聴数。
fn count_acceptance(hand_tiles: &[Tile], visible_counts: &[u8; 34], current_shanten: i32) -> u32 {
    let mut total = 0u32;
    for tile_type in 0..34u32 {
        // 場に4枚全て見えていたら受入不可
        let remaining = 4u8.saturating_sub(visible_counts[tile_type as usize]);
        if remaining == 0 {
            continue;
        }

        // この牌を加えて向聴数が下がるか（高速計算）
        let test_hand = Hand::new(hand_tiles.to_vec(), Some(Tile::new(tile_type)));
        let new_shanten = shanten_number(&test_hand);

        if new_shanten < current_shanten {
            total += remaining as u32;
        }
    }

    total
}

/// 手牌の推定打点を簡易計算する
fn estimate_hand_value(hand_tiles: &[Tile], state: &CpuGameState) -> f64 {
    let mut value = 0.0;

    // ドラ枚数
    let dora_count = count_dora_in_hand(hand_tiles, &state.dora_indicators);
    value += dora_count as f64 * 2.0;

    // 赤ドラ
    let red_count = hand_tiles.iter().filter(|t| t.is_red_dora()).count();
    value += red_count as f64 * 2.0;

    // 役牌（自風・場風・三元牌）の刻子候補
    let yakuhai_types = get_yakuhai_types(state.my_seat_wind, state.prevailing_wind);
    for &yh in &yakuhai_types {
        let count = hand_tiles.iter().filter(|t| t.get() == yh).count();
        if count >= 2 {
            value += count as f64 * 1.5;
        }
    }

    // タンヤオ可能性（中張牌のみ）
    let all_tanyao = hand_tiles.iter().all(|t| is_chunchanpai(t.get()));
    if all_tanyao {
        value += 1.5;
    }

    value
}

/// 手牌中のドラ枚数をカウント
fn count_dora_in_hand(hand_tiles: &[Tile], dora_indicators: &[Tile]) -> u32 {
    let mut count = 0u32;
    for indicator in dora_indicators {
        let dora_type = dora_indicator_to_dora(indicator.get());
        count += hand_tiles.iter().filter(|t| t.get() == dora_type).count() as u32;
    }
    count
}

/// 役牌となる牌種のリストを返す
fn get_yakuhai_types(seat_wind: Wind, prevailing_wind: Wind) -> Vec<TileType> {
    use mahjong_core::tile::Tile as T;
    let mut types = vec![T::Z5, T::Z6, T::Z7]; // 白發中

    // 場風
    let pw = match prevailing_wind {
        Wind::East => T::Z1,
        Wind::South => T::Z2,
        Wind::West => T::Z3,
        Wind::North => T::Z4,
    };
    types.push(pw);

    // 自風
    let sw = match seat_wind {
        Wind::East => T::Z1,
        Wind::South => T::Z2,
        Wind::West => T::Z3,
        Wind::North => T::Z4,
    };
    if !types.contains(&sw) {
        types.push(sw);
    }

    types
}

/// 中張牌（2-8）かどうか
fn is_chunchanpai(tile_type: TileType) -> bool {
    if tile_type >= 27 {
        return false; // 字牌
    }
    let num = tile_type % 9;
    num >= 1 && num <= 7 // 2-8 (0-indexed: 1-7)
}

/// 打牌候補から最良の1枚を選ぶ
pub fn select_best_discard(candidates: &[DiscardCandidate], config: &CpuConfig, attacking: bool) -> Option<Tile> {
    if candidates.is_empty() {
        return None;
    }

    let params = &config.params;
    let mut scored: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut score = 0.0;

            // 向聴数が低いほど良い（負の方向にスコアリング）
            score -= c.shanten as f64 * 100.0;

            // 有効牌数が多いほど良い
            score += c.acceptance_count as f64 * params.speed_weight;

            // 推定打点
            score += c.estimated_value * params.value_weight;

            // 守備中の場合は安全度を重視
            if !attacking {
                score += c.safety * params.retreat_threshold * 50.0;
            }

            (i, score)
        })
        .collect();

    // スコア降順でソート
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Weakレベルの場合、確率でランダム選択
    if config.level.should_make_mistake() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        // 簡易的な疑似ランダム（std のみ使用）
        let mut hasher = DefaultHasher::new();
        candidates.len().hash(&mut hasher);
        if let Some(drawn) = candidates.first() {
            drawn.tile.get().hash(&mut hasher);
        }
        let hash = hasher.finish();
        // 約30%の確率で2番目以降を選ぶ
        if hash % 100 < 30 && scored.len() > 1 {
            let idx = 1 + (hash as usize % (scored.len() - 1).max(1));
            let idx = idx.min(scored.len() - 1);
            return Some(candidates[scored[idx].0].tile);
        }
    }

    Some(candidates[scored[0].0].tile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::Tile;

    #[test]
    fn test_is_chunchanpai() {
        assert!(!is_chunchanpai(Tile::M1)); // 1m = 端牌
        assert!(is_chunchanpai(Tile::M2));  // 2m = 中張牌
        assert!(is_chunchanpai(Tile::M8));  // 8m = 中張牌
        assert!(!is_chunchanpai(Tile::M9)); // 9m = 端牌
        assert!(!is_chunchanpai(Tile::Z1)); // 東 = 字牌
    }

    #[test]
    fn test_count_dora_in_hand() {
        let hand = vec![Tile::new(Tile::M2), Tile::new(Tile::M2), Tile::new(Tile::M3)];
        let indicators = vec![Tile::new(Tile::M1)]; // ドラ表示牌1m → ドラは2m
        assert_eq!(count_dora_in_hand(&hand, &indicators), 2);
    }
}
