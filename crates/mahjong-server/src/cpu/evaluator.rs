//! Hand evaluation: shanten, tile acceptance, and estimated value for
//! each possible discard. Works purely on CpuGameState; never touches
//! server internals.

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{ShantenNumber, calc_shanten_number};
use mahjong_core::hand_info::meld::Meld;
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora_in};

use super::client::CpuConfig;
use super::defense;
use super::heuristics::{self, DiscardContext};
use super::state::CpuGameState;

/// Evaluation of discarding one tile.
#[derive(Debug, Clone)]
pub struct DiscardCandidate {
    /// The tile to discard
    pub tile: Tile,
    /// Shanten after the discard
    pub shanten: ShantenNumber,
    /// Remaining count of tiles that advance the hand (acceptance)
    pub acceptance_count: u32,
    /// Estimated hand value; higher is better
    pub estimated_value: f64,
    /// Safety, 0.0 (most dangerous) to 1.0 (safest)
    pub safety: f64,
}

/// Evaluates every discard candidate in the hand.
pub fn evaluate_discards(state: &CpuGameState, config: &CpuConfig) -> Vec<DiscardCandidate> {
    let mut all_tiles = state.my_hand.clone();
    if let Some(drawn) = state.my_drawn {
        all_tiles.push(drawn);
    }

    if all_tiles.is_empty() {
        return Vec::new();
    }

    let visible_counts = state.visible_tile_counts();
    // Melds must count as groups, or an open hand's shanten is
    // wildly overestimated.
    let melds = state.my_melds_for_analysis();
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (i, &tile) in all_tiles.iter().enumerate() {
        // Skip duplicate tiles, but keep the red five separate from the
        // normal five: their discard values differ.
        if !seen.insert((tile.get(), tile.is_red_dora())) {
            continue;
        }

        let mut remaining: Vec<Tile> = all_tiles.clone();
        remaining.remove(i);

        let hand = Hand::new_with_melds(remaining.clone(), melds.clone(), None);
        let shanten = calc_shanten_number(&hand);

        let acceptance_count = if config.level.uses_acceptance_count() {
            count_acceptance(&remaining, &melds, &visible_counts, shanten)
        } else {
            0
        };

        let estimated_value = if config.level.uses_value_estimation() {
            estimate_hand_value(&remaining, state)
        } else {
            0.0
        };

        // With heuristics enabled even the weak level computes safety
        // (#173/#174: full fold applies from weak up).
        let safety = if config.level.uses_defense() || config.heuristics_enabled {
            defense::evaluate_safety(tile, state, config)
        } else {
            0.5 // Neutral when defense is not considered.
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

/// Counts the remaining tiles that would advance the 13-tile hand.
///
/// `current_shanten` is passed in because the caller already computed it.
fn count_acceptance(
    hand_tiles: &[Tile],
    melds: &[Meld],
    visible_counts: &[u8; 34],
    current_shanten: ShantenNumber,
) -> u32 {
    let mut total = 0u32;
    for tile_type in 0..34u32 {
        let remaining = 4u8.saturating_sub(visible_counts[tile_type as usize]);
        if remaining == 0 {
            continue;
        }

        let test_hand = Hand::new_with_melds(
            hand_tiles.to_vec(),
            melds.to_vec(),
            Some(Tile::new(tile_type)),
        );
        let new_shanten = calc_shanten_number(&test_hand);

        if new_shanten < current_shanten {
            total += remaining as u32;
        }
    }

    total
}

/// Rough estimate of the hand's value.
pub(crate) fn estimate_hand_value(hand_tiles: &[Tile], state: &CpuGameState) -> f64 {
    let mut value = 0.0;

    let dora_count = count_dora_in_hand(hand_tiles, &state.dora_indicators, state.three_player);
    value += dora_count as f64 * 2.0;

    let red_count = hand_tiles.iter().filter(|t| t.is_red_dora()).count();
    value += red_count as f64 * 2.0;

    let yakuhai_types = get_yakuhai_types(state.my_seat_wind, state.round_wind);
    for &yh in &yakuhai_types {
        let count = hand_tiles.iter().filter(|t| t.get() == yh).count();
        if count >= 2 {
            value += count as f64 * 1.5;
        }
    }

    let all_tanyao = hand_tiles.iter().all(|t| !t.is_1_9_honour());
    if all_tanyao {
        value += 1.5;
    }

    value
}

/// Counts dora in the hand.
fn count_dora_in_hand(hand_tiles: &[Tile], dora_indicators: &[Tile], three_player: bool) -> u32 {
    let mut count = 0u32;
    for indicator in dora_indicators {
        let dora_type = dora_indicator_to_dora_in(indicator.get(), three_player);
        count += hand_tiles.iter().filter(|t| t.get() == dora_type).count() as u32;
    }
    count
}

/// Tile kinds that count as value honours (yakuhai) for this seat.
pub(crate) fn get_yakuhai_types(seat_wind: Wind, round_wind: Wind) -> Vec<TileType> {
    use mahjong_core::tile::Tile as T;
    // Dragons + round wind + seat wind; Wind discriminants equal the
    // corresponding tile kinds.
    let mut types = vec![T::Z5, T::Z6, T::Z7];
    types.push(round_wind as TileType);
    if seat_wind != round_wind {
        types.push(seat_wind as TileType);
    }
    types
}

/// Picks the best discard among the candidates.
pub fn select_best_discard(
    candidates: &[DiscardCandidate],
    config: &CpuConfig,
    attacking: bool,
    state: &CpuGameState,
) -> Option<Tile> {
    if candidates.is_empty() {
        return None;
    }

    let ctx = DiscardContext {
        state,
        config,
        attacking,
    };

    let params = &config.params;
    let mut scored: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut score = 0.0;

            score -= c.shanten.as_i32() as f64 * 100.0;

            score += c.acceptance_count as f64 * params.speed_weight;

            score += c.estimated_value * params.value_weight;

            if !attacking {
                score += c.safety * params.retreat_threshold * 50.0;
            }

            // Heuristic adjustments; only those enabled for the level apply.
            score += heuristics::discard_adjustment(&ctx, c);

            (i, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // The weak level sometimes plays a deliberate mistake.
    if config.level.should_make_mistake() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        // Cheap pseudo-randomness using std only.
        let mut hasher = DefaultHasher::new();
        candidates.len().hash(&mut hasher);
        if let Some(drawn) = candidates.first() {
            drawn.tile.get().hash(&mut hasher);
        }
        let hash = hasher.finish();
        // ~30% chance to pick a non-best candidate.
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

    // --- evaluate_discards with melds ---

    /// Regression: melds must count as groups in the shanten of an open
    /// hand. Shanten used to be computed on a meld-less `Hand`, wildly
    /// overestimating open hands.
    #[test]
    fn test_evaluate_discards_counts_melds_as_blocks() {
        use mahjong_core::hand_info::meld::{MeldFrom, MeldType};

        let mut state = CpuGameState::new();
        // 123m and 456m already called; hand 789p + 11z + 34s, drawn 7z.
        // Discarding 7z leaves a 2s/5s tenpai.
        state.my_hand = vec![
            Tile::new(Tile::P7),
            Tile::new(Tile::P8),
            Tile::new(Tile::P9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::S3),
            Tile::new(Tile::S4),
        ];
        state.my_drawn = Some(Tile::new(Tile::Z7));
        state.player_melds[0] = vec![
            Meld {
                tiles: vec![
                    Tile::new(Tile::M1),
                    Tile::new(Tile::M2),
                    Tile::new(Tile::M3),
                ],
                category: MeldType::Chi,
                from: MeldFrom::Previous,
                called_tile: None,
            },
            Meld {
                tiles: vec![
                    Tile::new(Tile::M4),
                    Tile::new(Tile::M5),
                    Tile::new(Tile::M6),
                ],
                category: MeldType::Chi,
                from: MeldFrom::Previous,
                called_tile: None,
            },
        ];

        let candidates = evaluate_discards(&state, &normal_config());
        let z7 = candidates
            .iter()
            .find(|c| c.tile.get() == Tile::Z7)
            .expect("7z は打牌候補にあるはず");
        assert!(
            z7.shanten.is_ready(),
            "副露を含めれば 7z 切りで聴牌のはず（shanten = {}）",
            z7.shanten
        );
    }

    // --- count_dora_in_hand ---

    #[test]
    fn test_count_dora_in_hand() {
        let hand = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        let indicators = vec![Tile::new(Tile::M1)]; // dora is 2m
        assert_eq!(count_dora_in_hand(&hand, &indicators, false), 2);
    }

    #[test]
    fn test_count_dora_in_hand_no_dora() {
        let hand = vec![Tile::new(Tile::M3), Tile::new(Tile::P5)];
        let indicators = vec![Tile::new(Tile::M1)]; // dora is 2m
        assert_eq!(count_dora_in_hand(&hand, &indicators, false), 0);
    }

    #[test]
    fn test_count_dora_in_hand_multiple_indicators() {
        let hand = vec![Tile::new(Tile::M2), Tile::new(Tile::P3)];
        // Indicators 1m (dora 2m) and 2p (dora 3p).
        let indicators = vec![Tile::new(Tile::M1), Tile::new(Tile::P2)];
        assert_eq!(count_dora_in_hand(&hand, &indicators, false), 2);
    }

    // --- get_yakuhai_types ---

    #[test]
    fn test_get_yakuhai_types_includes_dragons() {
        let types = get_yakuhai_types(Wind::East, Wind::East);
        assert!(types.contains(&Tile::Z5));
        assert!(types.contains(&Tile::Z6));
        assert!(types.contains(&Tile::Z7));
    }

    #[test]
    fn test_get_yakuhai_types_same_wind_no_duplicate() {
        // Seat wind == round wind must not duplicate the tile kind.
        let types = get_yakuhai_types(Wind::East, Wind::East);
        let count = types.iter().filter(|&&t| t == Tile::Z1).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_get_yakuhai_types_different_winds() {
        let types = get_yakuhai_types(Wind::South, Wind::East);
        assert!(types.contains(&Tile::Z1)); // round wind
        assert!(types.contains(&Tile::Z2)); // seat wind
    }

    #[test]
    fn test_get_yakuhai_types_all_winds() {
        for seat in [Wind::East, Wind::South, Wind::West, Wind::North] {
            for prev in [Wind::East, Wind::South, Wind::West, Wind::North] {
                let types = get_yakuhai_types(seat, prev);
                assert!(types.len() >= 4); // 3 dragons + round wind + seat wind (deduped)
            }
        }
    }

    // --- estimate_hand_value ---

    #[test]
    fn test_estimate_hand_value_with_dora() {
        let mut state = CpuGameState::new();
        state.dora_indicators = vec![Tile::new(Tile::M1)];
        let hand = vec![Tile::new(Tile::M2), Tile::new(Tile::M2)];
        let value = estimate_hand_value(&hand, &state);
        assert!(value >= 4.0); // two dora x 2.0
    }

    #[test]
    fn test_estimate_hand_value_with_red_dora() {
        let state = CpuGameState::new();
        let hand = vec![Tile::new_red(Tile::M5)];
        let value = estimate_hand_value(&hand, &state);
        assert!(value >= 2.0);
    }

    #[test]
    fn test_estimate_hand_value_tanyao_bonus() {
        let state = CpuGameState::new();
        let hand = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::P5),
            Tile::new(Tile::S6),
        ];
        let value_tanyao = estimate_hand_value(&hand, &state);
        // Compared against the same hand with a terminal mixed in.
        let hand_nontanyao = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::P5),
            Tile::new(Tile::S6),
        ];
        let value_non = estimate_hand_value(&hand_nontanyao, &state);
        assert!(value_tanyao > value_non);
    }

    #[test]
    fn test_estimate_hand_value_yakuhai_pair() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.round_wind = Wind::East;
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
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P1),
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::S1),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::M4),
        ];
        state.my_drawn = Some(Tile::new(Tile::M5));
        let result = evaluate_discards(&state, &normal_config());
        assert!(!result.is_empty());
        assert!(result.len() <= 14);
    }

    #[test]
    fn test_evaluate_discards_deduplicates_same_tile() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        let result = evaluate_discards(&state, &weak_config());
        assert!(result.iter().filter(|c| c.tile.get() == Tile::M1).count() <= 1);
    }

    #[test]
    fn test_evaluate_discards_keeps_red_and_normal_five_candidates() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new_red(Tile::M5),
            Tile::new(Tile::M5),
            Tile::new(Tile::M1),
        ];

        let result = evaluate_discards(&state, &weak_config());
        let five_candidates: Vec<&DiscardCandidate> =
            result.iter().filter(|c| c.tile.get() == Tile::M5).collect();

        assert_eq!(five_candidates.len(), 2);
        assert!(five_candidates.iter().any(|c| c.tile.is_red_dora()));
        assert!(five_candidates.iter().any(|c| !c.tile.is_red_dora()));
    }

    #[test]
    fn test_evaluate_discards_weak_skips_acceptance_and_value() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        let result = evaluate_discards(&state, &weak_config());
        // Weak skips acceptance and value estimation. Safety is still
        // computed with heuristics on (no riichi around -> 1.0).
        for c in &result {
            assert_eq!(c.acceptance_count, 0);
            assert_eq!(c.estimated_value, 0.0);
            assert_eq!(c.safety, 1.0);
        }
    }

    #[test]
    fn test_evaluate_discards_weak_without_heuristics_uses_neutral_safety() {
        // Weak with heuristics off keeps the neutral 0.5 safety.
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        let config = weak_config().without_heuristics();
        let result = evaluate_discards(&state, &config);
        for c in &result {
            assert_eq!(c.safety, 0.5);
        }
    }

    #[test]
    fn test_evaluate_discards_strong_uses_all_features() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::P4),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::S4),
            Tile::new(Tile::M6),
            Tile::new(Tile::M7),
            Tile::new(Tile::M8),
            Tile::new(Tile::Z1),
        ];
        state.my_drawn = Some(Tile::new(Tile::Z2));
        let result = evaluate_discards(&state, &strong_config());
        assert!(result.iter().any(|c| c.acceptance_count > 0));
    }

    // --- select_best_discard ---

    #[test]
    fn test_select_best_discard_empty() {
        let state = CpuGameState::new();
        assert!(select_best_discard(&[], &normal_config(), true, &state).is_none());
    }

    fn make_candidate(tile_type: u32, shanten_val: i32, safety: f64) -> DiscardCandidate {
        // ShantenNumber cannot be fabricated, so derive it from a real
        // hand; tests that need controlled shanten values build candidates
        // through evaluate_discards instead.
        let hand = Hand::new(vec![Tile::new(tile_type)], None);
        let shanten = calc_shanten_number(&hand);
        let _ = shanten_val;
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
        let state = CpuGameState::new();
        let c = make_candidate(Tile::Z1, 1, 0.5);
        let result = select_best_discard(&[c], &normal_config(), true, &state);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get(), Tile::Z1);
    }

    #[test]
    fn test_select_best_discard_prefers_lower_shanten() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::M5),
            Tile::new(Tile::M6),
            Tile::new(Tile::M7),
            Tile::new(Tile::M8),
            Tile::new(Tile::M9),
            Tile::new(Tile::P1),
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::Z1),
        ];
        state.my_drawn = Some(Tile::new(Tile::Z2));
        let candidates = evaluate_discards(&state, &weak_config());
        // Discarding either honour reaches tenpai, so one must be picked.
        let result = select_best_discard(&candidates, &normal_config(), true, &state);
        assert!(result.is_some());
        let best = result.unwrap();
        assert!(best.get() == Tile::Z1 || best.get() == Tile::Z2);
    }

    #[test]
    fn test_select_best_discard_defense_mode_prefers_safe_tile() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P1),
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::S1),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::M4),
        ];
        let candidates = evaluate_discards(&state, &weak_config());
        // Build two candidates equal except for safety; weak_config pins
        // safety to 0.5, so set the difference by hand.
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
        let result = select_best_discard(&[dangerous, safe], &normal_config(), false, &state);
        assert_eq!(result.unwrap().get(), Tile::Z3);
    }
}
