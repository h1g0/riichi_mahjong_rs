//! 手牌評価
//!
//! 各牌を捨てた場合の向聴数・有効牌数・推定打点を計算する。
//! 入力は CpuGameState のみ（サーバ内部にはアクセスしない）。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{ShantenNumber, calc_shanten_number};
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora};

use super::client::CpuConfig;
use super::defense;
use super::state::CpuGameState;

/// 牌1枚を捨てた場合の評価
#[derive(Debug, Clone)]
pub struct DiscardCandidate {
    /// 捨てる牌
    pub tile: Tile,
    /// 捨てた後の向聴数
    pub shanten: ShantenNumber,
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
        let shanten = calc_shanten_number(&hand);

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
fn count_acceptance(hand_tiles: &[Tile], visible_counts: &[u8; 34], current_shanten: ShantenNumber) -> u32 {
    let mut total = 0u32;
    for tile_type in 0..34u32 {
        // 場に4枚全て見えていたら受入不可
        let remaining = 4u8.saturating_sub(visible_counts[tile_type as usize]);
        if remaining == 0 {
            continue;
        }

        // この牌を加えて向聴数が下がるか（高速計算）
        let test_hand = Hand::new(hand_tiles.to_vec(), Some(Tile::new(tile_type)));
        let new_shanten = calc_shanten_number(&test_hand);

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
pub fn select_best_discard(
    candidates: &[DiscardCandidate],
    config: &CpuConfig,
    attacking: bool,
) -> Option<Tile> {
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
            score -= c.shanten.as_i32() as f64 * 100.0;

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

    use crate::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
    use crate::cpu::state::CpuGameState;

    fn normal_config() -> CpuConfig {
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced)
    }

    fn strong_config() -> CpuConfig {
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced)
    }

    fn weak_config() -> CpuConfig {
        CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced)
    }

    // --- is_chunchanpai ---

    #[test]
    fn test_is_chunchanpai() {
        assert!(!is_chunchanpai(Tile::M1));
        assert!(is_chunchanpai(Tile::M2));
        assert!(is_chunchanpai(Tile::M8));
        assert!(!is_chunchanpai(Tile::M9));
        assert!(!is_chunchanpai(Tile::Z1));
    }

    #[test]
    fn test_is_chunchanpai_all_suits() {
        // 各スートの1・9は端牌
        for t in [Tile::M1, Tile::P1, Tile::S1, Tile::M9, Tile::P9, Tile::S9] {
            assert!(!is_chunchanpai(t), "expected false for tile {t}");
        }
        // 各スートの2〜8は中張牌
        for t in [Tile::M2, Tile::P5, Tile::S8] {
            assert!(is_chunchanpai(t), "expected true for tile {t}");
        }
        // 字牌はすべて false
        for t in Tile::Z1..=Tile::Z7 {
            assert!(!is_chunchanpai(t), "expected false for honor tile {t}");
        }
    }

    // --- count_dora_in_hand ---

    #[test]
    fn test_count_dora_in_hand() {
        let hand = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        let indicators = vec![Tile::new(Tile::M1)]; // ドラ表示牌1m → ドラは2m
        assert_eq!(count_dora_in_hand(&hand, &indicators), 2);
    }

    #[test]
    fn test_count_dora_in_hand_no_dora() {
        let hand = vec![Tile::new(Tile::M3), Tile::new(Tile::P5)];
        let indicators = vec![Tile::new(Tile::M1)]; // ドラは2m
        assert_eq!(count_dora_in_hand(&hand, &indicators), 0);
    }

    #[test]
    fn test_count_dora_in_hand_multiple_indicators() {
        let hand = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::P3),
        ];
        // ドラ表示牌1m（ドラ2m）と2p（ドラ3p）
        let indicators = vec![Tile::new(Tile::M1), Tile::new(Tile::P2)];
        assert_eq!(count_dora_in_hand(&hand, &indicators), 2);
    }

    // --- get_yakuhai_types ---

    #[test]
    fn test_get_yakuhai_types_includes_dragons() {
        let types = get_yakuhai_types(Wind::East, Wind::East);
        assert!(types.contains(&Tile::Z5)); // 白
        assert!(types.contains(&Tile::Z6)); // 發
        assert!(types.contains(&Tile::Z7)); // 中
    }

    #[test]
    fn test_get_yakuhai_types_same_wind_no_duplicate() {
        // 自風=場風=東 のとき Z1 は重複しない
        let types = get_yakuhai_types(Wind::East, Wind::East);
        let count = types.iter().filter(|&&t| t == Tile::Z1).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_get_yakuhai_types_different_winds() {
        // 自風=南, 場風=東 のとき Z1(東) と Z2(南) の両方が含まれる
        let types = get_yakuhai_types(Wind::South, Wind::East);
        assert!(types.contains(&Tile::Z1)); // 東（場風）
        assert!(types.contains(&Tile::Z2)); // 南（自風）
    }

    #[test]
    fn test_get_yakuhai_types_all_winds() {
        // 各風の組み合わせでクラッシュしない
        for seat in [Wind::East, Wind::South, Wind::West, Wind::North] {
            for prev in [Wind::East, Wind::South, Wind::West, Wind::North] {
                let types = get_yakuhai_types(seat, prev);
                assert!(types.len() >= 4); // 三元牌3枚 + 場風 + 自風(重複除く)
            }
        }
    }

    // --- estimate_hand_value ---

    #[test]
    fn test_estimate_hand_value_with_dora() {
        let mut state = CpuGameState::new();
        state.dora_indicators = vec![Tile::new(Tile::M1)];
        let hand = vec![Tile::new(Tile::M2), Tile::new(Tile::M2)]; // ドラ2枚
        let value = estimate_hand_value(&hand, &state);
        assert!(value >= 4.0); // ドラ2枚 × 2.0 = 4.0
    }

    #[test]
    fn test_estimate_hand_value_with_red_dora() {
        let state = CpuGameState::new();
        let hand = vec![Tile::new_red(Tile::M5)]; // 赤ドラ1枚
        let value = estimate_hand_value(&hand, &state);
        assert!(value >= 2.0);
    }

    #[test]
    fn test_estimate_hand_value_tanyao_bonus() {
        let state = CpuGameState::new();
        // 全部中張牌でタンヤオ可能
        let hand = vec![
            Tile::new(Tile::M2), Tile::new(Tile::M3), Tile::new(Tile::M4),
            Tile::new(Tile::P5), Tile::new(Tile::S6),
        ];
        let value_tanyao = estimate_hand_value(&hand, &state);
        // 端牌を混ぜた場合と比較してボーナスがある
        let hand_nontanyao = vec![
            Tile::new(Tile::M1), Tile::new(Tile::M3), Tile::new(Tile::M4),
            Tile::new(Tile::P5), Tile::new(Tile::S6),
        ];
        let value_non = estimate_hand_value(&hand_nontanyao, &state);
        assert!(value_tanyao > value_non);
    }

    #[test]
    fn test_estimate_hand_value_yakuhai_pair() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.prevailing_wind = Wind::East;
        // 東が2枚（役牌候補）
        let hand = vec![Tile::new(Tile::Z1), Tile::new(Tile::Z1)];
        let value = estimate_hand_value(&hand, &state);
        assert!(value > 0.0);
    }

    // --- evaluate_discards ---

    #[test]
    fn test_evaluate_discards_empty_hand() {
        let state = CpuGameState::new();
        let result = evaluate_discards(&state, &normal_config());
        assert!(result.is_empty());
    }

    #[test]
    fn test_evaluate_discards_with_drawn_tile() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1), Tile::new(Tile::M2), Tile::new(Tile::M3),
            Tile::new(Tile::P1), Tile::new(Tile::P2), Tile::new(Tile::P3),
            Tile::new(Tile::S1), Tile::new(Tile::S2), Tile::new(Tile::S3),
            Tile::new(Tile::Z1), Tile::new(Tile::Z2), Tile::new(Tile::Z3),
            Tile::new(Tile::M4),
        ];
        state.my_drawn = Some(Tile::new(Tile::M5));
        let result = evaluate_discards(&state, &normal_config());
        assert!(!result.is_empty());
        // 14枚の手牌、捨て候補が返る
        assert!(result.len() <= 14);
    }

    #[test]
    fn test_evaluate_discards_deduplicates_same_tile() {
        let mut state = CpuGameState::new();
        // 同じ牌が複数あっても重複評価しない
        state.my_hand = vec![
            Tile::new(Tile::M1), Tile::new(Tile::M1), Tile::new(Tile::M1),
            Tile::new(Tile::M2), Tile::new(Tile::M3),
        ];
        let result = evaluate_discards(&state, &weak_config());
        // M1 は1候補のみ、M2、M3 でmax3候補
        assert!(result.iter().filter(|c| c.tile.get() == Tile::M1).count() <= 1);
    }

    #[test]
    fn test_evaluate_discards_weak_skips_acceptance_and_value() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1), Tile::new(Tile::M2), Tile::new(Tile::M3),
        ];
        let result = evaluate_discards(&state, &weak_config());
        // Weak: acceptance_count=0, estimated_value=0.0, safety=0.5
        for c in &result {
            assert_eq!(c.acceptance_count, 0);
            assert_eq!(c.estimated_value, 0.0);
            assert_eq!(c.safety, 0.5);
        }
    }

    #[test]
    fn test_evaluate_discards_strong_uses_all_features() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M2), Tile::new(Tile::M3), Tile::new(Tile::M4),
            Tile::new(Tile::P2), Tile::new(Tile::P3), Tile::new(Tile::P4),
            Tile::new(Tile::S2), Tile::new(Tile::S3), Tile::new(Tile::S4),
            Tile::new(Tile::M6), Tile::new(Tile::M7), Tile::new(Tile::M8),
            Tile::new(Tile::Z1),
        ];
        state.my_drawn = Some(Tile::new(Tile::Z2));
        let result = evaluate_discards(&state, &strong_config());
        // Strong: acceptance_count > 0 の候補が存在する
        assert!(result.iter().any(|c| c.acceptance_count > 0));
    }

    // --- select_best_discard ---

    #[test]
    fn test_select_best_discard_empty() {
        assert!(select_best_discard(&[], &normal_config(), true).is_none());
    }

    fn make_candidate(tile_type: u32, shanten_val: i32, safety: f64) -> DiscardCandidate {
        // 実際の Hand から ShantenNumber を計算して取得する
        let hand = Hand::new(vec![Tile::new(tile_type)], None);
        let shanten = calc_shanten_number(&hand);
        // shanten の値は hand 依存なので、score 比較のためにフィールドを上書きできない。
        // 代わりに evaluate_discards 経由で取得した候補を使うテストで検証する。
        // ここでは safety と estimated_value のみを制御したいので、
        // 向聴数が同じになるよう設計した手牌から candidates を生成して使う。
        let _ = shanten_val; // 以下のテストでは state 経由で生成するため使わない
        DiscardCandidate {
            tile: Tile::new(tile_type),
            shanten,
            acceptance_count: 0,
            estimated_value: 0.0,
            safety,
        }
    }

    #[test]
    fn test_select_best_discard_single_candidate() {
        let c = make_candidate(Tile::Z1, 1, 0.5);
        let result = select_best_discard(&[c], &normal_config(), true);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get(), Tile::Z1);
    }

    #[test]
    fn test_select_best_discard_prefers_lower_shanten() {
        // テンパイ形（M1M2M3 M4M5M6 M7M8M9 P1P2P3 Z1）から Z1 を捨てるとテンパイ
        // evaluate_discards 経由で shanten 付きの候補を生成する
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1), Tile::new(Tile::M2), Tile::new(Tile::M3),
            Tile::new(Tile::M4), Tile::new(Tile::M5), Tile::new(Tile::M6),
            Tile::new(Tile::M7), Tile::new(Tile::M8), Tile::new(Tile::M9),
            Tile::new(Tile::P1), Tile::new(Tile::P2), Tile::new(Tile::P3),
            Tile::new(Tile::Z1),
        ];
        state.my_drawn = Some(Tile::new(Tile::Z2));
        let candidates = evaluate_discards(&state, &weak_config());
        // Z1 か Z2 を捨てたときがテンパイ（shanten=0）になるはず
        let result = select_best_discard(&candidates, &normal_config(), true);
        assert!(result.is_some());
        let best = result.unwrap();
        // Z1 か Z2 を選ぶ（字牌を捨ててテンパイ）
        assert!(best.get() == Tile::Z1 || best.get() == Tile::Z2);
    }

    #[test]
    fn test_select_best_discard_defense_mode_prefers_safe_tile() {
        // 向聴数は同じで safety だけ異なる2候補を手動構成
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1), Tile::new(Tile::M2), Tile::new(Tile::M3),
            Tile::new(Tile::P1), Tile::new(Tile::P2), Tile::new(Tile::P3),
            Tile::new(Tile::S1), Tile::new(Tile::S2), Tile::new(Tile::S3),
            Tile::new(Tile::Z1), Tile::new(Tile::Z2), Tile::new(Tile::Z3),
            Tile::new(Tile::M4),
        ];
        let candidates = evaluate_discards(&state, &weak_config());
        // safety フィールドを上書きした2候補を作る
        // （weak_config では safety=0.5 固定なので手動で差をつける）
        let shanten_base = candidates.first().map(|c| c.shanten).unwrap();
        let dangerous = DiscardCandidate {
            tile: Tile::new(Tile::M4),
            shanten: shanten_base,
            acceptance_count: 0,
            estimated_value: 0.0,
            safety: 0.0,
        };
        let safe = DiscardCandidate {
            tile: Tile::new(Tile::Z3),
            shanten: shanten_base,
            acceptance_count: 0,
            estimated_value: 0.0,
            safety: 1.0,
        };
        // 守備モードでは安全な牌を選ぶ
        let result = select_best_discard(&[dangerous, safe], &normal_config(), false);
        assert_eq!(result.unwrap().get(), Tile::Z3);
    }
}
