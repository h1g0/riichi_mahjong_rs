//! Unit tests for the heuristics.

use super::*;
use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;
use mahjong_core::tile::Tile;

use crate::cpu::client::CpuPersonality;

fn make_candidate(tile_type: u32) -> DiscardCandidate {
    let hand = Hand::new(vec![Tile::new(tile_type)], None);
    DiscardCandidate {
        tile: Tile::new(tile_type),
        shanten: calc_shanten_number(&hand),
        acceptance_count: 0,
        estimated_value: 0.0,
        safety: 0.5,
    }
}

fn fixed_bonus_heuristic(
    name: &'static str,
    min_level: CpuLevel,
    apply: fn(&DiscardContext, &DiscardCandidate) -> f64,
) -> DiscardHeuristic {
    DiscardHeuristic {
        name,
        min_level,
        apply,
    }
}

#[test]
fn test_registry_returns_zero_when_disabled() {
    // With heuristics disabled the real registry always yields 0 (A/B baseline).
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[Tile::M1, Tile::Z3, Tile::M5, Tile::M6]);
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    for t in [Tile::M1, Tile::Z3, Tile::M5] {
        assert_eq!(discard_adjustment(&ctx, &make_candidate(t)), 0.0);
    }
}

#[test]
fn test_level_gating() {
    let heuristics = [
        fixed_bonus_heuristic("weak-rule", CpuLevel::Weak, |_, _| 1.0),
        fixed_bonus_heuristic("normal-rule", CpuLevel::Normal, |_, _| 10.0),
        fixed_bonus_heuristic("strong-rule", CpuLevel::Strong, |_, _| 100.0),
    ];
    let state = CpuGameState::new();
    let candidate = make_candidate(Tile::M1);

    // Weak: only weak-and-up heuristics.
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 1.0);

    // Normal: weak-and-up plus normal-and-up.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 11.0);

    // Strong: everything.
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    assert_eq!(
        discard_adjustment_with(&heuristics, &ctx, &candidate),
        111.0
    );
}

#[test]
fn test_heuristics_disabled_config_returns_zero() {
    // heuristics_enabled=false disables everything (A/B baseline).
    let heuristics = [fixed_bonus_heuristic(
        "weak-rule",
        CpuLevel::Weak,
        |_, _| 1.0,
    )];
    let state = CpuGameState::new();
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    let candidate = make_candidate(Tile::M1);
    assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 0.0);
}

// --- Discard heuristics ---

fn attack_ctx<'a>(state: &'a CpuGameState, config: &'a CpuConfig) -> DiscardContext<'a> {
    DiscardContext {
        state,
        config,
        attacking: true,
    }
}

#[test]
fn test_isolated_tile_bonus_ordering() {
    // Isolated-tile discard order: guest wind > 1/9 > value honour > 2/8 > inside.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::East;
    state.round_wind = Wind::East;
    // Z3 guest wind, Z5 value honour, M1/S8 isolated terminals, M5 isolated inside, P7P7 pair.
    state.my_hand = tiles(&[
        Tile::Z3,
        Tile::Z5,
        Tile::M1,
        Tile::S8,
        Tile::M5,
        Tile::P7,
        Tile::P7,
    ]);
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    let bonus = |t: u32| isolated_tile_bonus(&ctx, &make_candidate(t));

    assert!(bonus(Tile::Z3) > bonus(Tile::M1), "客風 > 1/9");
    assert!(bonus(Tile::M1) > bonus(Tile::Z5), "1/9 > 役牌");
    assert!(bonus(Tile::Z5) > bonus(Tile::S8), "役牌 > 2/8");
    assert!(bonus(Tile::S8) > bonus(Tile::M5), "2/8 > 中張");
    assert_eq!(bonus(Tile::M5), 0.0, "孤立中張牌は雑に切らない");
    assert_eq!(bonus(Tile::P7), 0.0, "対子は孤立牌ではない");
}

#[test]
fn test_isolated_tile_bonus_requires_isolation() {
    // A neighbour within two ranks means not isolated.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[Tile::M1, Tile::M3, Tile::M9, Tile::S9]);
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    // M1 forms a closed-shape candidate with M3, so it is not isolated.
    assert_eq!(isolated_tile_bonus(&ctx, &make_candidate(Tile::M1)), 0.0);
    // M9 and S9 are isolated.
    assert!(isolated_tile_bonus(&ctx, &make_candidate(Tile::M9)) > 0.0);
    assert!(isolated_tile_bonus(&ctx, &make_candidate(Tile::S9)) > 0.0);
}

#[test]
fn test_shape_protection_bonus() {
    // Two-sided shapes score negative (kept); edge/closed positive (shed).
    let mut state = CpuGameState::new();
    // M2M3 two-sided, P1P2 edge, S3S5 closed, S8S8 pair.
    state.my_hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::P1,
        Tile::P2,
        Tile::S3,
        Tile::S5,
        Tile::S8,
        Tile::S8,
    ]);
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    let bonus = |t: u32| shape_protection_bonus(&ctx, &make_candidate(t));

    assert!(bonus(Tile::M2) < 0.0, "両面の牌は守る");
    assert!(bonus(Tile::M3) < 0.0, "両面の牌は守る");
    assert!(bonus(Tile::P1) > 0.0, "辺張は整理しやすい");
    assert!(bonus(Tile::S3) > 0.0, "嵌張は整理しやすい");
    assert!(bonus(Tile::S5) > 0.0, "嵌張は整理しやすい");
    assert_eq!(bonus(Tile::S8), 0.0, "対子は対象外");
}

#[test]
fn test_shape_protection_inactive_when_defending() {
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[Tile::M2, Tile::M3]);
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(shape_protection_bonus(&ctx, &make_candidate(Tile::M2)), 0.0);
}

#[test]
fn test_dora_protection_bonus() {
    let mut state = CpuGameState::new();
    state.dora_indicators = vec![Tile::new(Tile::P8)]; // dora is P9
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    // Dora carries a penalty.
    assert!(dora_protection_bonus(&ctx, &make_candidate(Tile::P9)) < 0.0);
    // Non-dora is unaffected.
    assert_eq!(dora_protection_bonus(&ctx, &make_candidate(Tile::S9)), 0.0);

    // Red fives too.
    let red_five = DiscardCandidate {
        tile: Tile::new_red(Tile::M5),
        ..make_candidate(Tile::M5)
    };
    assert!(dora_protection_bonus(&ctx, &red_five) < 0.0);

    // No adjustment while defending; safety takes over.
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(
        dora_protection_bonus(&defending, &make_candidate(Tile::P9)),
        0.0
    );
}

#[test]
fn test_defense_safety_bonus() {
    let state = CpuGameState::new();
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);

    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;

    // No adjustment while attacking.
    let attacking = attack_ctx(&state, &config);
    assert_eq!(defense_safety_bonus(&attacking, &candidate), 0.0);

    // While defending, safety weighs 300 (three shanten steps).
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 300.0);

    // Genbutsu (1.0) vs off-suji inside (0.15) exceeds two shanten steps.
    candidate.safety = 0.15;
    let dangerous = defense_safety_bonus(&defending, &candidate);
    assert!(300.0 - dangerous > 200.0);
}

// --- Block theory (#149 #150 #151 #153) ---

/// Six-block hand: 2 groups + closed + two-sided x2 + pair.
fn six_block_state() -> CpuGameState {
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M7,
        Tile::M8,
        Tile::M9,
        Tile::P1,
        Tile::P3,
        Tile::S6,
        Tile::S7,
        Tile::S2,
        Tile::S3,
        Tile::Z5,
    ]);
    state.my_drawn = Some(Tile::new(Tile::Z5));
    state
}

#[test]
fn test_count_blocks() {
    let state = six_block_state();
    // M234 + M789 + P1P3 + S67 + S23 + Z5Z5 = six blocks.
    assert_eq!(count_blocks(&state), 6);

    // Five-block hand.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::M9,
        Tile::M9,
        Tile::P9,
        Tile::Z3,
    ]);
    state.my_drawn = Some(Tile::new(Tile::S9));
    assert_eq!(count_blocks(&state), 4); // 3 groups + pair; floaters do not count
}

#[test]
fn test_five_block_bonus_dismantles_weak_blocks() {
    let state = six_block_state();
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    let bonus = |t: u32| five_block_bonus(&ctx, &make_candidate(t));

    // Closed-shape tiles may be shed.
    assert!(bonus(Tile::P1) > 0.0);
    assert!(bonus(Tile::P3) > 0.0);
    // Two-sided shapes are exempt.
    assert_eq!(bonus(Tile::S6), 0.0);
    assert_eq!(bonus(Tile::S2), 0.0);
    // A single pair is not surplus.
    assert_eq!(bonus(Tile::Z5), 0.0);
}

#[test]
fn test_five_block_bonus_inactive_under_six_blocks() {
    // At five blocks or fewer even closed shapes get no bonus.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M7,
        Tile::M8,
        Tile::M9,
        Tile::P1,
        Tile::P3,
        Tile::S6,
        Tile::S7,
        Tile::Z5,
    ]);
    state.my_drawn = Some(Tile::new(Tile::Z5));
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    assert_eq!(five_block_bonus(&ctx, &make_candidate(Tile::P1)), 0.0);
}

#[test]
fn test_five_block_bonus_surplus_pair() {
    // At six blocks with two pairs, shedding a pair is allowed.
    let mut state = six_block_state();
    // Turn S23 into S3S3 for a second pair (M234 M789 P1P3 S67 S3S3 Z5Z5).
    state.my_hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M7,
        Tile::M8,
        Tile::M9,
        Tile::P1,
        Tile::P3,
        Tile::S6,
        Tile::S7,
        Tile::S3,
        Tile::S3,
        Tile::Z5,
    ]);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    assert!(five_block_bonus(&ctx, &make_candidate(Tile::S3)) > 0.0);
    assert!(five_block_bonus(&ctx, &make_candidate(Tile::Z5)) > 0.0);
}

#[test]
fn test_sole_pair_protection() {
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::M9,
        Tile::M9,
        Tile::P9,
        Tile::Z3,
    ]);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    // The only pair is protected.
    assert!(sole_pair_protection(&ctx, &make_candidate(Tile::M9)) < 0.0);
    // Non-pairs are exempt.
    assert_eq!(sole_pair_protection(&ctx, &make_candidate(Tile::P9)), 0.0);

    // Two pairs lift the protection.
    state.my_hand.push(Tile::new(Tile::P9));
    let ctx = attack_ctx(&state, &config);
    assert_eq!(sole_pair_protection(&ctx, &make_candidate(Tile::M9)), 0.0);

    // A triplet supplies another head candidate, lifting the protection.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[Tile::M9, Tile::M9, Tile::S5, Tile::S5, Tile::S5, Tile::P2]);
    let ctx = attack_ctx(&state, &config);
    assert_eq!(sole_pair_protection(&ctx, &make_candidate(Tile::M9)), 0.0);
}

#[test]
fn test_dead_shape_bonus_kanchan() {
    let mut state = CpuGameState::new();
    // S2S4 closed shape waiting on S3.
    state.my_hand = tiles(&[Tile::S2, Tile::S4, Tile::M5, Tile::M5]);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // No S3 visible: the shape is alive.
    let ctx = attack_ctx(&state, &config);
    assert_eq!(dead_shape_bonus(&ctx, &make_candidate(Tile::S2)), 0.0);

    // Three S3 visible: the shape is dead.
    state.all_discards[1] = tiles(&[Tile::S3, Tile::S3, Tile::S3]);
    let ctx = attack_ctx(&state, &config);
    assert!(dead_shape_bonus(&ctx, &make_candidate(Tile::S2)) > 0.0);
    assert!(dead_shape_bonus(&ctx, &make_candidate(Tile::S4)) > 0.0);
    // Pairs are exempt.
    assert_eq!(dead_shape_bonus(&ctx, &make_candidate(Tile::M5)), 0.0);
}

#[test]
fn test_dead_shape_bonus_ryanmen_both_waits_dead() {
    let mut state = CpuGameState::new();
    // S6S7 two-sided shape waiting on S5/S8.
    state.my_hand = tiles(&[Tile::S6, Tile::S7]);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    let ctx = attack_ctx(&state, &config);
    assert_eq!(dead_shape_bonus(&ctx, &make_candidate(Tile::S6)), 0.0);

    // Four S5 and three S8 visible leave one wait: nearly dead.
    state.all_discards[1] = tiles(&[
        Tile::S5,
        Tile::S5,
        Tile::S5,
        Tile::S5,
        Tile::S8,
        Tile::S8,
        Tile::S8,
    ]);
    let ctx = attack_ctx(&state, &config);
    assert!(dead_shape_bonus(&ctx, &make_candidate(Tile::S6)) > 0.0);
}

#[test]
fn test_excess_pair_bonus() {
    let mut state = CpuGameState::new();
    // 3 pairs + 2 groups: standard 1-shanten vs seven pairs 2 - lean standard.
    state.my_hand = tiles(&[
        Tile::M5,
        Tile::M5,
        Tile::M9,
        Tile::M9,
        Tile::Z2,
        Tile::Z2,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::S1,
    ]);
    state.my_drawn = Some(Tile::new(Tile::S9));

    // Strong breaks pairs in order inside > terminal > honour (0).
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    let m5 = excess_pair_bonus(&ctx, &make_candidate(Tile::M5));
    let m9 = excess_pair_bonus(&ctx, &make_candidate(Tile::M9));
    let z2 = excess_pair_bonus(&ctx, &make_candidate(Tile::Z2));
    assert!(m5 > m9, "中張牌対子からほぐす");
    assert!(m9 > z2, "字牌対子は残す");
    assert_eq!(z2, 0.0);

    // Non-pair tiles are exempt.
    assert_eq!(excess_pair_bonus(&ctx, &make_candidate(Tile::P4)), 0.0);
}

#[test]
fn test_excess_pair_bonus_inactive_when_seven_pairs_close() {
    let mut state = CpuGameState::new();
    // Five pairs: seven pairs is at least as close, so no break-up.
    state.my_hand = tiles(&[
        Tile::M5,
        Tile::M5,
        Tile::M9,
        Tile::M9,
        Tile::Z2,
        Tile::Z2,
        Tile::P2,
        Tile::P2,
        Tile::S8,
        Tile::S8,
        Tile::S1,
        Tile::M1,
        Tile::P9,
    ]);
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    assert_eq!(excess_pair_bonus(&ctx, &make_candidate(Tile::M5)), 0.0);
}

#[test]
fn test_excess_pair_bonus_requires_three_pairs() {
    let mut state = CpuGameState::new();
    // Two pairs are never broken up.
    state.my_hand = tiles(&[
        Tile::M5,
        Tile::M5,
        Tile::M9,
        Tile::M9,
        Tile::P4,
        Tile::P5,
        Tile::P6,
    ]);
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    assert_eq!(excess_pair_bonus(&ctx, &make_candidate(Tile::M5)), 0.0);
}

// --- Route selection: seven pairs / toitoi (#154 #155 #156 #157) ---

#[test]
fn test_preferred_form_normal_under_four_pairs() {
    // #154: three pairs never make seven pairs the main route.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::Z1,
        Tile::Z1,
        Tile::Z2,
        Tile::Z2,
        Tile::P3,
        Tile::P3,
        Tile::M8,
        Tile::M9,
        Tile::S4,
        Tile::S5,
        Tile::M1,
        Tile::P9,
        Tile::S9,
    ]);
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_preferred_form_seven_pairs_with_stiff_pairs() {
    // #156: four stiff pairs (honours/terminals/isolated) lean seven pairs.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::Z1,
        Tile::Z1,
        Tile::Z5,
        Tile::Z5,
        Tile::M9,
        Tile::M9,
        Tile::P1,
        Tile::P1,
        Tile::S4,
        Tile::S5,
        Tile::M2,
        Tile::P5,
        Tile::S9,
    ]);
    assert_eq!(preferred_form(&state), Form::SevenPairs);
}

#[test]
fn test_preferred_form_normal_with_flexible_pairs() {
    // #155: consecutive pairs (M334455) extend well as sequences - standard.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::M3,
        Tile::M3,
        Tile::M4,
        Tile::M4,
        Tile::M5,
        Tile::M5,
        Tile::P6,
        Tile::P6,
        Tile::S2,
        Tile::S3,
        Tile::S7,
        Tile::S8,
        Tile::Z3,
    ]);
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_preferred_form_normal_with_melds() {
    // Any meld rules out seven pairs.
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[
        Tile::Z1,
        Tile::Z1,
        Tile::Z5,
        Tile::Z5,
        Tile::M9,
        Tile::M9,
        Tile::P1,
        Tile::P1,
        Tile::S4,
        Tile::S5,
    ]);
    state.player_melds[0] = vec![pon_meld(Tile::S9)];
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_is_stiff_pair() {
    let mut counts = [0u8; 34];
    counts[Tile::Z1 as usize] = 2; // honour pair
    counts[Tile::M9 as usize] = 2; // terminal pair
    counts[Tile::P5 as usize] = 2; // isolated inside pair
    counts[Tile::S5 as usize] = 2; // inside pair with neighbours
    counts[Tile::S6 as usize] = 1;

    assert!(is_stiff_pair(&counts, Tile::Z1));
    assert!(is_stiff_pair(&counts, Tile::M9));
    assert!(is_stiff_pair(&counts, Tile::P5));
    assert!(!is_stiff_pair(&counts, Tile::S5), "S6が隣にあるので伸びる");
}

#[test]
fn test_route_lock_penalizes_off_route_discards() {
    // #154: even at three pairs (where seven pairs is closer) the route
    // is standard, and shape-breaking discards get penalized
    let mut state = CpuGameState::new();
    // Pairs Z1Z1 Z2Z2 P3P3 + shapes M8M9 S4S5 + floaters.
    state.my_hand = tiles(&[
        Tile::Z1,
        Tile::Z1,
        Tile::Z2,
        Tile::Z2,
        Tile::P3,
        Tile::P3,
        Tile::M8,
        Tile::M9,
        Tile::S4,
        Tile::S5,
        Tile::M1,
        Tile::P9,
        Tile::S9,
    ]);
    state.my_drawn = Some(Tile::new(Tile::M5));
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    // Breaking the M8M9 shape hurts the standard route: big penalty.
    let break_taatsu = route_lock_bonus(&ctx, &make_candidate(Tile::M8));
    // Discarding a floater loses nothing either way: no penalty.
    let cut_float = route_lock_bonus(&ctx, &make_candidate(Tile::M1));
    assert!(
        break_taatsu < cut_float,
        "ターツ壊し({break_taatsu}) < 浮き牌切り({cut_float}) のはず"
    );
    assert_eq!(cut_float, 0.0);
}

#[test]
fn test_route_lock_follows_seven_pairs_route() {
    // On the seven-pairs route, pair-breaking discards (drifting towards
    // the standard form) are penalized. The base score already punishes
    // shanten loss, so this uses a hand with a standard-form backup where
    // the discard keeps the shanten and only the route lock differs.
    let mut state = CpuGameState::new();
    // Four stiff pairs + a group + a shape: both forms 2-shanten.
    state.my_hand = tiles(&[
        Tile::Z1,
        Tile::Z1,
        Tile::Z5,
        Tile::Z5,
        Tile::M9,
        Tile::M9,
        Tile::P1,
        Tile::P1,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::S7,
        Tile::S8,
    ]);
    state.my_drawn = Some(Tile::new(Tile::M2));
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    assert_eq!(preferred_form(&state), Form::SevenPairs);

    // Breaking a pair keeps the overall shanten (standard stays 2)
    // but drifts from seven pairs: penalty.
    let break_pair = route_lock_bonus(&ctx, &make_candidate(Tile::Z1));
    // A floater discard loses nothing either way.
    let cut_float = route_lock_bonus(&ctx, &make_candidate(Tile::M2));
    assert!(
        break_pair < cut_float,
        "対子壊し({break_pair}) < 浮き牌切り({cut_float}) のはず"
    );
    assert_eq!(cut_float, 0.0);
}

#[test]
fn test_toitoi_prospect_by_blocks() {
    // #157 (normal+): toitoi needs melds + pairs/triplets >= 4 blocks.
    let seat = Wind::East;
    let prev = Wind::East;

    // Three blocks (2 melds + 1 pair) plus floaters: no prospect.
    let hand = tiles(&[Tile::P1, Tile::P1, Tile::M2, Tile::S3, Tile::M6, Tile::P7]);
    let melds = vec![pon_meld(Tile::M9), pon_meld(Tile::S9)];
    assert!(!has_yaku_prospect(&hand, &melds, seat, prev, true));

    // Four blocks (2 melds + 2 pairs): toitoi prospect.
    let hand = tiles(&[Tile::P1, Tile::P1, Tile::S3, Tile::S3, Tile::M2, Tile::M6]);
    assert!(has_yaku_prospect(&hand, &melds, seat, prev, true));

    // Weak (legacy rule): prospect with two or fewer floaters.
    assert!(has_yaku_prospect(&hand, &melds, seat, prev, false));
}

// --- Smarter calling (#164 #165) ---

#[test]
fn test_tanyao_prospect_strict_conditions() {
    // #164 (normal+): kuitan prospect needs <=2 orphans and multiple
    // blocks inside the tanyao range.
    let seat = Wind::East;
    let prev = Wind::East;
    let melds = vec![chi_meld(Tile::S2)]; // S234, inside tiles only

    // Three orphans: prospect under the loose rule, none under strict.
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::P4,
        Tile::P5,
        Tile::S6,
        Tile::M9,
        Tile::P9,
        Tile::S9,
    ]);
    assert!(has_yaku_prospect(&hand, &melds, seat, prev, false));
    assert!(!has_yaku_prospect(&hand, &melds, seat, prev, true));

    // Two orphans + two tanyao-range blocks (M2M3, P4P5): prospect even strict.
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::P4,
        Tile::P5,
        Tile::S6,
        Tile::M9,
        Tile::P9,
    ]);
    assert!(has_yaku_prospect(&hand, &melds, seat, prev, true));

    // No orphans but scattered tanyao range (<=1 block): no prospect.
    let hand = tiles(&[Tile::M2, Tile::M5, Tile::P5, Tile::S8]);
    assert!(!has_yaku_prospect(&hand, &melds, seat, prev, true));
}

#[test]
fn test_cheap_distant_call_detection() {
    // #165: 2+ shanten, no value element, non-dealer: cheap and distant.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    let melds = vec![chi_meld(Tile::S2)];
    // Scattered three-suit hand above 2-shanten, no dora or value honours.
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::P4,
        Tile::P5,
        Tile::S6,
        Tile::S7,
        Tile::M6,
        Tile::P8,
        Tile::S4,
        Tile::M7,
    ]);
    assert!(is_cheap_distant_call(&state, &hand, &melds, false));

    // The dealer is exempt.
    let mut dealer_state = CpuGameState::new();
    dealer_state.my_seat_wind = Wind::East;
    assert!(!is_cheap_distant_call(&dealer_state, &hand, &melds, false));

    // Dora counts as a value element.
    let mut dora_state = CpuGameState::new();
    dora_state.my_seat_wind = Wind::South;
    dora_state.dora_indicators = vec![Tile::new(Tile::M1)]; // dora M2 is in hand
    assert!(!is_cheap_distant_call(&dora_state, &hand, &melds, false));

    // A value-honour pair counts as a value element.
    let hand_with_yakuhai = tiles(&[
        Tile::Z5,
        Tile::Z5,
        Tile::M2,
        Tile::M3,
        Tile::P4,
        Tile::P5,
        Tile::S6,
        Tile::S7,
        Tile::M6,
        Tile::P8,
    ]);
    assert!(!is_cheap_distant_call(
        &state,
        &hand_with_yakuhai,
        &melds,
        false
    ));
}

#[test]
fn test_cheap_distant_call_requires_two_shanten() {
    // A call reaching 1-shanten or better is not distant.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    let melds = vec![chi_meld(Tile::S2)];
    // 2 groups + pair + shape: about 1-shanten after the chii.
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S6,
        Tile::S6,
        Tile::M6,
        Tile::M7,
    ]);
    assert!(!is_cheap_distant_call(&state, &hand, &melds, false));
}

// --- Thirteen Orphans route (#158 #159 #160 #161) ---

/// Builds a 13-tile hand with n orphan kinds padded with inside tiles.
fn orphan_hand(kinds: usize) -> Vec<Tile> {
    let fillers = [Tile::M4, Tile::M5, Tile::P5, Tile::S5, Tile::S6, Tile::P3];
    let mut hand: Vec<Tile> = ORPHAN_TYPES
        .iter()
        .take(kinds)
        .map(|&t| Tile::new(t))
        .collect();
    hand.extend(fillers.iter().take(13 - kinds).map(|&t| Tile::new(t)));
    hand
}

#[test]
fn test_preferred_form_kokushi_with_ten_kinds() {
    // #160: 10+ orphan kinds make Thirteen Orphans the main route.
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(10);
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);
}

#[test]
fn test_preferred_form_kokushi_nine_kinds_when_closer() {
    // #158: 9 kinds qualify when clearly closer than the other forms.
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(9);
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);
}

#[test]
fn test_preferred_form_normal_with_seven_kinds_and_decent_hand() {
    // #158: 7 kinds do not qualify while the normal hand has prospects.
    let mut state = CpuGameState::new();
    let mut hand: Vec<Tile> = ORPHAN_TYPES.iter().take(7).map(|&t| Tile::new(t)).collect();
    hand.extend(tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::P6,
    ]));
    state.my_hand = hand;
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_kokushi_route_abandoned_when_missing_type_dead() {
    // #161: a missing requirement with 4 visible makes the form impossible.
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(10); // holds no Z5/Z6/Z7
    state.all_discards[1] = vec![Tile::new(Tile::Z5); 4];
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_kokushi_route_abandoned_when_needed_tiles_thin_late() {
    // #161: from mid-game, two missing kinds with <=1 copy left abandon it.
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(10);
    state.all_discards[1] = vec![
        Tile::new(Tile::Z5),
        Tile::new(Tile::Z5),
        Tile::new(Tile::Z5),
        Tile::new(Tile::Z6),
        Tile::new(Tile::Z6),
    ];
    state.all_discards[2] = vec![Tile::new(Tile::Z6)];

    // Turn 1 is too early to abandon.
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);

    // From turn 7 the chase is abandoned.
    state.all_discards[0] = vec![Tile::new(Tile::P5); 6];
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_is_far_behind() {
    // 16000+ points behind the leader.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::East;
    state.scores = [8000, 42000, 25000, 25000];
    assert!(is_far_behind(&state));

    // Flat scores.
    state.scores = [25000; 4];
    assert!(!is_far_behind(&state));

    // A close last place does not qualify.
    state.scores = [20000, 26000, 27000, 27000];
    assert!(!is_far_behind(&state));

    // Last place, 8000+ behind.
    state.scores = [17000, 27000, 28000, 28000];
    assert!(is_far_behind(&state));
}

#[test]
fn test_preferred_form_kokushi_seven_kinds_when_far_behind() {
    // #159: when far behind, chase from 7 kinds even if slightly farther.
    // Compare identical hands with only the score situation changed.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::East;
    let mut hand: Vec<Tile> = ORPHAN_TYPES.iter().take(7).map(|&t| Tile::new(t)).collect();
    hand.extend(tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::S5,
    ]));
    state.my_hand = hand;

    // Flat scores: no chase (the orphans are farther than standard).
    state.scores = [25000; 4];
    assert_ne!(preferred_form(&state), Form::ThirteenOrphans);

    // A distant last place chases.
    state.scores = [5000, 45000, 25000, 25000];
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);
}

// --- Push/fold (#178) and mawashi (#179) ---

#[test]
fn test_judge_push_folds_cheap_bad_shape_tenpai() {
    // #178: a bad-shape cheap tenpai folds against a threat.
    let mut state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    state.my_seat_wind = Wind::South; // avoid the dealer exemption
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Fold);

    // The weak level is exempt.
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);
}

#[test]
fn test_judge_push_pushes_good_shape_tenpai() {
    // #178: a good-shape tenpai pushes one riichi even when cheap.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);
}

#[test]
fn test_judge_push_pushes_high_value_against_multiple_threats() {
    // #178: a mangan-class good shape pushes even two riichi.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)]; // three dora
    state.player_riichi[2] = true;
    state.player_riichi[3] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 2), PushJudgement::Push);
}

#[test]
fn test_judge_push_dealer_pushes_good_shape() {
    // #178: the dealer pushes a good shape even against two riichi.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::East;
    state.player_riichi[2] = true;
    state.player_riichi[3] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 2), PushJudgement::Push);
}

#[test]
fn test_judge_push_two_shanten_with_value() {
    // #178: a mangan-class 2-shanten pushes a single threat.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.my_hand = tiles(&[
        Tile::M3,
        Tile::M4,
        Tile::M5,
        Tile::M5,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S6,
        Tile::S7,
        Tile::S2,
        Tile::S2,
        Tile::Z3,
        Tile::Z4,
    ]);
    state.dora_indicators = vec![Tile::new(Tile::M4), Tile::new(Tile::M4)]; // M5 double dora
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);

    // A cheap dora-less 2-shanten defers to the legacy retreat judgement.
    state.dora_indicators = vec![];
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);
}

#[test]
fn test_judge_push_neutral_without_threats() {
    let state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 0), PushJudgement::Neutral);
}

#[test]
fn test_mawashi_reduces_safety_weight_when_close() {
    // #179: Strong lowers the safety weight when folding at tenpai/1-shanten.
    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;

    // A tenpai hand with plenty of wall left.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.remaining_tiles = 40;

    // Strong at tenpai: weight 150 (mawashi).
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 150.0);

    // Normal always folds fully (weight 300).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 300.0);

    // Even Strong folds fully when the hand is far (weight 300).
    let mut far_state = CpuGameState::new();
    far_state.remaining_tiles = 40;
    far_state.my_hand = tiles(&[
        Tile::M1,
        Tile::M4,
        Tile::M7,
        Tile::P2,
        Tile::P5,
        Tile::P8,
        Tile::S3,
        Tile::S6,
        Tile::S9,
        Tile::Z1,
        Tile::Z2,
        Tile::Z3,
        Tile::Z4,
    ]);
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &far_state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 300.0);
}

#[test]
fn test_keishiki_tenpai_weights_at_endgame() {
    // #184/#185: near the draw, tenpai/1-shanten chases formal tenpai.
    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;

    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.scores = [26000, 25000, 25000, 24000]; // we (South) are second
    state.remaining_tiles = 6;
    state.round_number = 1;
    state.total_rounds = 4;

    // #184 (normal+): near the draw, weight 150.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 150.0);

    // #185 (strong): final hand, not leading: weight 120.
    state.round_number = 3;
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 120.0);

    // #185: the dealer also values keeping tenpai.
    let mut dealer_state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    dealer_state.my_seat_wind = Wind::East;
    dealer_state.remaining_tiles = 6;
    let defending = DiscardContext {
        state: &dealer_state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 120.0);
}

// --- Endgame and score situation (#183-#191) ---

#[test]
fn test_last_discard_safety_bonus() {
    // #186: the final discard weighs safety even while attacking.
    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // With wall left, no adjustment.
    let mut state = CpuGameState::new();
    state.remaining_tiles = 10;
    state.player_riichi[1] = true;
    let ctx = attack_ctx(&state, &config);
    assert_eq!(last_discard_safety_bonus(&ctx, &candidate), 0.0);

    // Empty wall + a riichi: adjusted even while attacking.
    state.remaining_tiles = 0;
    let ctx = attack_ctx(&state, &config);
    assert_eq!(last_discard_safety_bonus(&ctx, &candidate), 200.0);

    // No threat, no adjustment.
    let mut state = CpuGameState::new();
    state.remaining_tiles = 0;
    let ctx = attack_ctx(&state, &config);
    assert_eq!(last_discard_safety_bonus(&ctx, &candidate), 0.0);
}

#[test]
fn test_cheap_call_allowed_when_final_round_speed_matters() {
    // #187: in the final hand a cheap win that climbs a place lifts
    // the suppression.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.scores = [25000, 24000, 26000, 25000]; // we (South) are third by 2000
    state.round_number = 3;
    state.total_rounds = 4;
    let melds = vec![chi_meld(Tile::S2)];
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::P4,
        Tile::P5,
        Tile::S6,
        Tile::S7,
        Tile::M6,
        Tile::P8,
        Tile::S4,
        Tile::M7,
    ]);
    // Normally cheap-and-distant, but the final-hand margin allows it.
    assert!(!is_cheap_distant_call(&state, &hand, &melds, false));

    // Outside the final hand it is suppressed.
    state.round_number = 1;
    assert!(is_cheap_distant_call(&state, &hand, &melds, false));
}

#[test]
fn test_cheap_call_suppressed_when_mangan_needed() {
    // #187: needing a mangan-class win, even a close (1-shanten) cheap
    // call is suppressed.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.scores = [25000, 15000, 35000, 25000]; // we (South) trail the leader by 20000
    state.round_number = 3;
    state.total_rounds = 4;
    let melds = vec![chi_meld(Tile::S2)];
    // About 1-shanten after the chii (2 groups + pair + shape).
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S6,
        Tile::S6,
        Tile::M6,
        Tile::M7,
    ]);
    assert!(is_cheap_distant_call(&state, &hand, &melds, false));

    // With flat scores a 1-shanten call is not suppressed.
    state.round_number = 1;
    assert!(!is_cheap_distant_call(&state, &hand, &melds, false));
}

#[test]
fn test_cheap_call_allowed_with_large_stakes() {
    // #191 (strong): big stakes allow cheap calls.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.riichi_sticks = 2; // 2000 points on the table
    let melds = vec![chi_meld(Tile::S2)];
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::P4,
        Tile::P5,
        Tile::S6,
        Tile::S7,
        Tile::M6,
        Tile::P8,
        Tile::S4,
        Tile::M7,
    ]);
    // Strong considers the stakes: allowed.
    assert!(!is_cheap_distant_call(&state, &hand, &melds, true));
    // Normal ignores them: still suppressed.
    assert!(is_cheap_distant_call(&state, &hand, &melds, false));
}

#[test]
fn test_judge_push_top_in_second_half_folds() {
    // #188: a second-half leader folds even a cheap good-shape tenpai.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.scores = [25000, 40000, 20000, 15000]; // we (South) lead by far
    state.round_number = 3;
    state.total_rounds = 4;
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Fold);

    // Only a mangan-class good shape pushes.
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)]; // three dora
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);

    // First half keeps the normal judgement (good shape + one threat: push).
    let mut early = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    early.my_seat_wind = Wind::South;
    early.scores = [25000, 40000, 20000, 15000];
    early.round_number = 0;
    early.total_rounds = 4;
    early.player_riichi[2] = true;
    let ctx = CallContext {
        state: &early,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);
}

#[test]
fn test_judge_push_far_behind_lowers_value_threshold() {
    // #189: far behind, the 2-shanten push bar drops.
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.my_hand = tiles(&[
        Tile::M3,
        Tile::M4,
        Tile::M5,
        Tile::M5,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S6,
        Tile::S7,
        Tile::S2,
        Tile::S2,
        Tile::Z3,
        Tile::Z4,
    ]);
    state.dora_indicators = vec![Tile::new(Tile::M4)]; // two M5 dora, ~4.0 value
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // Flat scores: below the 6.0 bar, Neutral.
    state.scores = [25000; 4];
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);

    // Distant last place: the bar drops and it pushes.
    state.scores = [42000, 8000, 25000, 25000];
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);
}

#[test]
fn test_judge_push_dealer_keeps_pushing_cheap_tenpai() {
    // #190: the dealer's bad-shape cheap tenpai defers against one threat.
    let mut state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    state.my_seat_wind = Wind::East;
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    // Non-dealer folds; the dealer is Neutral (legacy pushes).
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);
}

#[test]
fn test_judge_push_stakes_keep_cheap_tenpai_alive() {
    // #191 (strong): big stakes remove the fold recommendation.
    let mut state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    state.my_seat_wind = Wind::South;
    state.riichi_sticks = 2;
    state.player_riichi[2] = true;

    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);

    // Normal ignores the stakes: Fold.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Fold);
}

#[test]
fn test_judge_riichi_far_behind_declares_over_damaten() {
    // #189: far behind, even a locked mangan declares instead of damaten.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)]; // three dora
    state.scores = [42000, 8000, 25000, 25000]; // we (South) are a distant last
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

// --- Riichi vs damaten (#168-#172) ---

/// Builds a judgement state: a tenpai 13 tiles plus a floater to tsumogiri.
fn riichi_state(hand: &[u32], drawn: u32) -> CpuGameState {
    let mut state = CpuGameState::new();
    state.my_hand = tiles(hand);
    state.my_drawn = Some(Tile::new(drawn));
    state
}

/// Yakuless tenpai: closed wait on M8, no pinfu or tanyao.
const NO_YAKU_TENPAI: [u32; 13] = [
    Tile::M2,
    Tile::M3,
    Tile::M4,
    Tile::P4,
    Tile::P5,
    Tile::P6,
    Tile::S4,
    Tile::S5,
    Tile::S6,
    Tile::M7,
    Tile::M9,
    Tile::Z3,
    Tile::Z3,
];

/// Two-sided tenpai with tanyao + pinfu locked (waiting M3/M6).
const GOOD_SHAPE_TENPAI: [u32; 13] = [
    Tile::P2,
    Tile::P3,
    Tile::P4,
    Tile::P5,
    Tile::P6,
    Tile::P7,
    Tile::S3,
    Tile::S4,
    Tile::S5,
    Tile::S8,
    Tile::S8,
    Tile::M4,
    Tile::M5,
];

/// Tanyao-only closed-wait tenpai (waiting M7).
const CHEAP_KANCHAN_TENPAI: [u32; 13] = [
    Tile::M2,
    Tile::M3,
    Tile::M4,
    Tile::P4,
    Tile::P5,
    Tile::P6,
    Tile::S4,
    Tile::S5,
    Tile::S6,
    Tile::M6,
    Tile::M8,
    Tile::S2,
    Tile::S2,
];

#[test]
fn test_judge_riichi_declares_with_no_yaku() {
    // #168: a yakuless tenpai cannot win without riichi: declare.
    let state = riichi_state(&NO_YAKU_TENPAI, Tile::Z4);
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

#[test]
fn test_judge_riichi_damaten_with_mangan() {
    // #170: damaten mangan on every wait (tanyao+pinfu+3 dora): damaten.
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    // Dora: two S8 (indicator S7) + one M4 (indicator M3) = 3.
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)];

    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Damaten);

    // Weak skips #170 and declares as an uncontested good shape (#169).
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

#[test]
fn test_judge_riichi_declares_good_shape() {
    // #169: an uncontested good shape declares even when cheap.
    let state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

#[test]
fn test_judge_riichi_cheap_kanchan_depends_on_turn() {
    // #171: a cheap bad shape declares early and uncontested,
    // stays damaten later.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // Turn 1: declare.
    let state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);

    // Turn 11: damaten.
    let mut state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    state.all_discards[0] = vec![Tile::new(Tile::Z4); 10];
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Damaten);
}

#[test]
fn test_judge_riichi_strong_defers_with_many_upgrades() {
    // #172: Strong waits a turn on an early bad shape with many upgrades.
    let state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);

    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Damaten);

    // Normal skips #172 and declares early uncontested (#171).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

#[test]
fn test_judge_riichi_neutral_when_disabled() {
    let state = riichi_state(&NO_YAKU_TENPAI, Tile::Z4);
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Neutral);
}

#[test]
fn test_estimate_ron_han() {
    // tanyao + pinfu + 3 dora = 5 han
    let state = {
        let mut s = CpuGameState::new();
        s.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)];
        s
    };
    let remaining = tiles(&GOOD_SHAPE_TENPAI);
    let han = estimate_ron_han(&state, &remaining, &[], Tile::M3);
    assert_eq!(han, Some(5));

    // No yaku: None.
    let state = CpuGameState::new();
    let remaining = tiles(&NO_YAKU_TENPAI);
    assert_eq!(estimate_ron_han(&state, &remaining, &[], Tile::M8), None);
}

// --- has_yaku_prospect ---

fn tiles(types: &[u32]) -> Vec<Tile> {
    types.iter().map(|&t| Tile::new(t)).collect()
}

fn pon_meld(tile_type: u32) -> Meld {
    Meld {
        tiles: vec![Tile::new(tile_type); 3],
        category: MeldType::Pon,
        from: MeldFrom::Unknown,
        called_tile: Some(Tile::new(tile_type)),
    }
}

fn chi_meld(start: u32) -> Meld {
    Meld {
        tiles: vec![Tile::new(start), Tile::new(start + 1), Tile::new(start + 2)],
        category: MeldType::Chi,
        from: MeldFrom::Previous,
        called_tile: Some(Tile::new(start)),
    }
}

#[test]
fn test_yaku_prospect_yakuhai_pair() {
    // A White dragon pair gives a value-honour prospect.
    let hand = tiles(&[Tile::Z5, Tile::Z5, Tile::M1, Tile::M9, Tile::P1, Tile::S9]);
    let melds = vec![chi_meld(Tile::P2)];
    assert!(has_yaku_prospect(
        &hand,
        &melds,
        Wind::East,
        Wind::East,
        false
    ));
}

#[test]
fn test_yaku_prospect_tanyao() {
    // Inside-only melds and hand give a tanyao prospect.
    let hand = tiles(&[Tile::M2, Tile::M3, Tile::P4, Tile::P5, Tile::S6, Tile::M9]);
    let melds = vec![chi_meld(Tile::S2)];
    assert!(has_yaku_prospect(
        &hand,
        &melds,
        Wind::East,
        Wind::East,
        false
    ));
}

#[test]
fn test_yaku_prospect_honitsu() {
    // Characters plus honours only: a flush prospect.
    let hand = tiles(&[Tile::M1, Tile::M2, Tile::M3, Tile::M7, Tile::Z2, Tile::Z3]);
    let melds = vec![chi_meld(Tile::M4)];
    assert!(has_yaku_prospect(
        &hand,
        &melds,
        Wind::East,
        Wind::East,
        false
    ));
}

#[test]
fn test_yaku_prospect_toitoi() {
    // All-triplet melds with a pair-heavy hand: a toitoi prospect.
    let hand = tiles(&[Tile::M9, Tile::M9, Tile::P1, Tile::P1, Tile::S9]);
    let melds = vec![pon_meld(Tile::M1), pon_meld(Tile::S1)];
    assert!(has_yaku_prospect(
        &hand,
        &melds,
        Wind::East,
        Wind::East,
        false
    ));
}

#[test]
fn test_yaku_prospect_none_for_junk_hand() {
    // Three scattered suits, orphan meld, no value honours: no prospect.
    let hand = tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P3,
        Tile::P4,
        Tile::P5,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::S2,
        Tile::S7,
    ]);
    let melds = vec![pon_meld(Tile::M9)];
    assert!(!has_yaku_prospect(
        &hand,
        &melds,
        Wind::East,
        Wind::East,
        false
    ));
}

// --- judge_pon ---

fn call_state_with_hand(hand: Vec<Tile>) -> CpuGameState {
    let mut state = CpuGameState::new();
    state.my_hand = hand;
    state
}

#[test]
fn test_judge_pon_forbids_yakuless_call() {
    // 3 groups + M9 pair: the pon lowers shanten but leaves no yaku.
    let state = call_state_with_hand(tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P3,
        Tile::P4,
        Tile::P5,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::M9,
        Tile::M9,
        Tile::S2,
        Tile::S7,
    ]));
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_pon(&ctx, Tile::new(Tile::M9)), CallJudgement::Forbid);
}

#[test]
fn test_judge_pon_forbids_fourth_meld() {
    // Already three melds: a pon into a bare pair is forbidden.
    let mut state = call_state_with_hand(tiles(&[Tile::S3, Tile::S3, Tile::M5, Tile::M9]));
    state.player_melds[0] = vec![chi_meld(Tile::M1), pon_meld(Tile::P5), pon_meld(Tile::S9)];
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_pon(&ctx, Tile::new(Tile::S3)), CallJudgement::Forbid);
}

#[test]
fn test_judge_pon_encourages_yakuhai() {
    // A White dragon pon is encouraged.
    let state = call_state_with_hand(tiles(&[
        Tile::Z5,
        Tile::Z5,
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S2,
        Tile::S2,
        Tile::M7,
        Tile::M8,
        Tile::S9,
    ]));
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(
        judge_pon(&ctx, Tile::new(Tile::Z5)),
        CallJudgement::Encourage
    );
}

#[test]
fn test_judge_pon_neutral_for_tanyao_call() {
    // An inside pon in an inside-heavy hand is neutral.
    let state = call_state_with_hand(tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P3,
        Tile::P4,
        Tile::P5,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::S3,
        Tile::S3,
        Tile::M5,
        Tile::M6,
    ]));
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_pon(&ctx, Tile::new(Tile::S3)), CallJudgement::Neutral);
}

#[test]
fn test_judge_pon_neutral_when_heuristics_disabled() {
    // With heuristics off, even three melds stay Neutral (legacy).
    let mut state = call_state_with_hand(tiles(&[Tile::S3, Tile::S3, Tile::M5, Tile::M9]));
    state.player_melds[0] = vec![chi_meld(Tile::M1), pon_meld(Tile::P5), pon_meld(Tile::S9)];
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_pon(&ctx, Tile::new(Tile::S3)), CallJudgement::Neutral);
}

// --- judge_chi ---

#[test]
fn test_judge_chi_forbids_yakuless_call() {
    // In a scattered three-suit hand an orphan chii leaves no yaku.
    // Hand: M789 + P345 + S456 + M9M9 + S2 S7; chii M9 with M7M8.
    let state = call_state_with_hand(tiles(&[
        Tile::M7,
        Tile::M8,
        Tile::P3,
        Tile::P4,
        Tile::P5,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::M1,
        Tile::M1,
        Tile::S2,
        Tile::S7,
        Tile::Z2,
    ]));
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    // The M9 chii kills tanyao (orphan in the meld), no value honours,
    // three suits: Forbid.
    assert_eq!(
        judge_chi(
            &ctx,
            Tile::new(Tile::M9),
            [Tile::new(Tile::M7), Tile::new(Tile::M8)]
        ),
        CallJudgement::Forbid
    );
}

#[test]
fn test_judge_chi_neutral_for_tanyao_call() {
    // An inside-only chii keeps the tanyao prospect: Neutral.
    let state = call_state_with_hand(tiles(&[
        Tile::M3,
        Tile::M4,
        Tile::P3,
        Tile::P4,
        Tile::P5,
        Tile::S4,
        Tile::S5,
        Tile::S6,
        Tile::S3,
        Tile::S3,
        Tile::M5,
        Tile::M6,
        Tile::M7,
    ]));
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(
        judge_chi(
            &ctx,
            Tile::new(Tile::M5),
            [Tile::new(Tile::M3), Tile::new(Tile::M4)]
        ),
        CallJudgement::Neutral
    );
}

// --- judge_ankan ---

/// A hand a concealed kan would break: the four S5 serve a sequence
/// plus a triplet in a tenpai shape.
fn hand_breaking_kan_state() -> CpuGameState {
    let mut state = call_state_with_hand(tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::P2,
        Tile::P3,
        Tile::P4,
        Tile::S4,
        Tile::S5,
        Tile::S5,
        Tile::S5,
        Tile::S6,
        Tile::Z1,
        Tile::Z3,
    ]));
    state.my_drawn = Some(Tile::new(Tile::S5));
    state
}

#[test]
fn test_judge_ankan_forbids_hand_breaking_kan() {
    // The four S5 form S456 + S555 at tenpai; the kan drops to 1-shanten.
    let state = hand_breaking_kan_state();
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_ankan(&ctx, Tile::S5), CallJudgement::Forbid);
}

#[test]
fn test_judge_ankan_neutral_for_weak_level() {
    // Weak is exempt, allowing beginner-style careless kans.
    let state = hand_breaking_kan_state();
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_ankan(&ctx, Tile::S5), CallJudgement::Neutral);
}

/// A 1-shanten hand where the kan keeps the shanten (four P2 as a
/// floating triplet plus one).
fn shanten_keeping_kan_state() -> CpuGameState {
    let mut state = call_state_with_hand(tiles(&[
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M6,
        Tile::M7,
        Tile::S3,
        Tile::S3,
        Tile::P2,
        Tile::P2,
        Tile::P2,
        Tile::P2,
        Tile::Z1,
        Tile::Z2,
    ]));
    state.my_drawn = Some(Tile::new(Tile::M5));
    state
}

#[test]
fn test_judge_ankan_neutral_when_shanten_kept_and_no_riichi() {
    let state = shanten_keeping_kan_state();
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_ankan(&ctx, Tile::P2), CallJudgement::Neutral);
}

#[test]
fn test_judge_ankan_forbids_kan_during_opponent_riichi_without_tenpai() {
    // Under an opponent's riichi and not tenpai after the kan:
    // the new-dora risk is not worth it.
    let mut state = shanten_keeping_kan_state();
    state.player_riichi[2] = true; // West declared; we are East
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_ankan(&ctx, Tile::P2), CallJudgement::Forbid);
}

#[test]
fn test_heuristic_can_reference_candidate_and_context() {
    // A heuristic may read both the candidate and the context.
    let heuristics = [fixed_bonus_heuristic(
        "honour-in-defense",
        CpuLevel::Weak,
        |ctx, c| {
            if !ctx.attacking && c.tile.get() >= 27 {
                50.0
            } else {
                0.0
            }
        },
    )];
    let state = CpuGameState::new();
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);

    let honour = make_candidate(Tile::Z1);
    let number = make_candidate(Tile::M5);

    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(
        discard_adjustment_with(&heuristics, &defending, &honour),
        50.0
    );
    assert_eq!(
        discard_adjustment_with(&heuristics, &defending, &number),
        0.0
    );

    let attacking = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    assert_eq!(
        discard_adjustment_with(&heuristics, &attacking, &honour),
        0.0
    );
}
