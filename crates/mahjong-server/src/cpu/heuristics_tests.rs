//! 定石（heuristics）のユニットテスト

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
    // 定石無効なら実レジストリでも補正は常に0（A/Bベースライン）
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

    // Weak: 弱以上の定石のみ
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 1.0);

    // Normal: 弱以上 + 中以上
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = DiscardContext {
        state: &state,
        config: &config,
        attacking: true,
    };
    assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 11.0);

    // Strong: 全て
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
    // heuristics_enabled=false なら全定石が無効（新旧比較用のベースライン）
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

// --- 打牌定石 ---

fn attack_ctx<'a>(state: &'a CpuGameState, config: &'a CpuConfig) -> DiscardContext<'a> {
    DiscardContext {
        state,
        config,
        attacking: true,
    }
}

#[test]
fn test_isolated_tile_bonus_ordering() {
    // 孤立牌の切りやすさ: 客風 > 1/9 > 役牌 > 2/8 > 中張牌
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::East;
    state.round_wind = Wind::East;
    // Z3=客風(西), Z5=白(役牌), M1=孤立1, S8=孤立8, M5=孤立中張, P7P7=対子
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
    // 前後2つ以内に牌があれば孤立ではない
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[Tile::M1, Tile::M3, Tile::M9, Tile::S9]);
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    // M1 は M3 と嵌張候補 → 孤立ではない
    assert_eq!(isolated_tile_bonus(&ctx, &make_candidate(Tile::M1)), 0.0);
    // M9 / S9 は孤立
    assert!(isolated_tile_bonus(&ctx, &make_candidate(Tile::M9)) > 0.0);
    assert!(isolated_tile_bonus(&ctx, &make_candidate(Tile::S9)) > 0.0);
}

#[test]
fn test_shape_protection_bonus() {
    // 両面はマイナス（守る）、辺張・嵌張はプラス（整理しやすい）
    let mut state = CpuGameState::new();
    // M2M3=両面, P1P2=辺張, S3S5=嵌張, S8S8=対子
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
    state.dora_indicators = vec![Tile::new(Tile::P8)]; // ドラは P9
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    // ドラはペナルティ
    assert!(dora_protection_bonus(&ctx, &make_candidate(Tile::P9)) < 0.0);
    // 非ドラは補正なし
    assert_eq!(dora_protection_bonus(&ctx, &make_candidate(Tile::S9)), 0.0);

    // 赤ドラもペナルティ
    let red_five = DiscardCandidate {
        tile: Tile::new_red(Tile::M5),
        ..make_candidate(Tile::M5)
    };
    assert!(dora_protection_bonus(&ctx, &red_five) < 0.0);

    // 守備時は補正なし（安全度を優先）
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

    // 攻撃中は補正なし
    let attacking = attack_ctx(&state, &config);
    assert_eq!(defense_safety_bonus(&attacking, &candidate), 0.0);

    // 守備時は安全度に大きな重み（向聴数3段階分 = 300）
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 300.0);

    // 現物(1.0)と無筋中張牌(0.15)の差は向聴数2段階を超える
    candidate.safety = 0.15;
    let dangerous = defense_safety_bonus(&defending, &candidate);
    assert!(300.0 - dangerous > 200.0);
}

// --- ブロック理論（#149 #150 #151 #153）---

/// 6ブロックの手牌（2面子 + 嵌張 + 両面 + 両面 + 対子）
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
    // M234 + M789 + P1P3 + S67 + S23 + Z5Z5 = 6ブロック
    assert_eq!(count_blocks(&state), 6);

    // 5ブロックの手牌
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
    assert_eq!(count_blocks(&state), 4); // 3面子 + 対子（浮き牌は数えない）
}

#[test]
fn test_five_block_bonus_dismantles_weak_blocks() {
    let state = six_block_state();
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);

    let bonus = |t: u32| five_block_bonus(&ctx, &make_candidate(t));

    // 嵌張の構成牌は整理対象
    assert!(bonus(Tile::P1) > 0.0);
    assert!(bonus(Tile::P3) > 0.0);
    // 両面は対象外
    assert_eq!(bonus(Tile::S6), 0.0);
    assert_eq!(bonus(Tile::S2), 0.0);
    // 対子は1つしかないので余剰対子扱いしない
    assert_eq!(bonus(Tile::Z5), 0.0);
}

#[test]
fn test_five_block_bonus_inactive_under_six_blocks() {
    // 5ブロック以下なら嵌張でも補正しない
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
    // 6ブロックで対子が2つあれば、対子の整理も許容する
    let mut state = six_block_state();
    // S23 を S3S3 に変えて対子2つに（M234 M789 P1P3 S67 S3S3 Z5Z5）
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

    // 唯一の対子は保護
    assert!(sole_pair_protection(&ctx, &make_candidate(Tile::M9)) < 0.0);
    // 対子以外は対象外
    assert_eq!(sole_pair_protection(&ctx, &make_candidate(Tile::P9)), 0.0);

    // 対子が2つあれば保護しない
    state.my_hand.push(Tile::new(Tile::P9));
    let ctx = attack_ctx(&state, &config);
    assert_eq!(sole_pair_protection(&ctx, &make_candidate(Tile::M9)), 0.0);

    // 刻子があれば雀頭候補は他にもあるので保護しない
    let mut state = CpuGameState::new();
    state.my_hand = tiles(&[Tile::M9, Tile::M9, Tile::S5, Tile::S5, Tile::S5, Tile::P2]);
    let ctx = attack_ctx(&state, &config);
    assert_eq!(sole_pair_protection(&ctx, &make_candidate(Tile::M9)), 0.0);
}

#[test]
fn test_dead_shape_bonus_kanchan() {
    let mut state = CpuGameState::new();
    // S2S4 の嵌張（待ちは S3）
    state.my_hand = tiles(&[Tile::S2, Tile::S4, Tile::M5, Tile::M5]);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // S3 が見えていない → 生きたターツ
    let ctx = attack_ctx(&state, &config);
    assert_eq!(dead_shape_bonus(&ctx, &make_candidate(Tile::S2)), 0.0);

    // S3 が3枚見えている → 死にターツ
    state.all_discards[1] = tiles(&[Tile::S3, Tile::S3, Tile::S3]);
    let ctx = attack_ctx(&state, &config);
    assert!(dead_shape_bonus(&ctx, &make_candidate(Tile::S2)) > 0.0);
    assert!(dead_shape_bonus(&ctx, &make_candidate(Tile::S4)) > 0.0);
    // 対子側は対象外
    assert_eq!(dead_shape_bonus(&ctx, &make_candidate(Tile::M5)), 0.0);
}

#[test]
fn test_dead_shape_bonus_ryanmen_both_waits_dead() {
    let mut state = CpuGameState::new();
    // S6S7 の両面（待ちは S5/S8）
    state.my_hand = tiles(&[Tile::S6, Tile::S7]);
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    let ctx = attack_ctx(&state, &config);
    assert_eq!(dead_shape_bonus(&ctx, &make_candidate(Tile::S6)), 0.0);

    // S5 が4枚 + S8 が3枚見えている → 残り1枚 → ほぼ死に
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
    // 3対子 + 2面子: 一般形1向聴、七対子2向聴 → 一般形寄り
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

    // 強レベル: 中張牌対子 > 1/9対子 > 字牌対子(0) の順でほぐす
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = attack_ctx(&state, &config);
    let m5 = excess_pair_bonus(&ctx, &make_candidate(Tile::M5));
    let m9 = excess_pair_bonus(&ctx, &make_candidate(Tile::M9));
    let z2 = excess_pair_bonus(&ctx, &make_candidate(Tile::Z2));
    assert!(m5 > m9, "中張牌対子からほぐす");
    assert!(m9 > z2, "字牌対子は残す");
    assert_eq!(z2, 0.0);

    // 対子でない牌は対象外
    assert_eq!(excess_pair_bonus(&ctx, &make_candidate(Tile::P4)), 0.0);
}

#[test]
fn test_excess_pair_bonus_inactive_when_seven_pairs_close() {
    let mut state = CpuGameState::new();
    // 5対子: 七対子2向聴 <= 一般形 → ほぐさない
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
    // 2対子ではほぐさない
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

// --- 七対子・対々和の路線判断（#154 #155 #156 #157）---

#[test]
fn test_preferred_form_normal_under_four_pairs() {
    // #154: 3対子では七対子を本線にしない
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
    // #156: 字牌・么九牌・孤立対子が4つ → 七対子寄り
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
    // #155: 連続対子（M334455）は順子手としての伸びが強い → 一般形
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
    // 副露があれば七対子は不可能
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
    counts[Tile::Z1 as usize] = 2; // 字牌対子
    counts[Tile::M9 as usize] = 2; // 么九対子
    counts[Tile::P5 as usize] = 2; // 孤立した中張対子
    counts[Tile::S5 as usize] = 2; // 周囲に牌がある中張対子
    counts[Tile::S6 as usize] = 1;

    assert!(is_stiff_pair(&counts, Tile::Z1));
    assert!(is_stiff_pair(&counts, Tile::M9));
    assert!(is_stiff_pair(&counts, Tile::P5));
    assert!(!is_stiff_pair(&counts, Tile::S5), "S6が隣にあるので伸びる");
}

#[test]
fn test_route_lock_penalizes_off_route_discards() {
    // #154: 3対子（七対子の方が向聴数は近い）でも一般形を選び、
    // ターツを壊す打牌（七対子追従）にペナルティを与える
    let mut state = CpuGameState::new();
    // 対子: Z1Z1 Z2Z2 P3P3（3つ）+ ターツ M8M9 S4S5 + 浮き牌
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

    // ターツ(M8M9)を壊す打牌は一般形視点で損 → 大きなペナルティ
    let break_taatsu = route_lock_bonus(&ctx, &make_candidate(Tile::M8));
    // 浮き牌を切る打牌は両形でロスなし → ペナルティなし
    let cut_float = route_lock_bonus(&ctx, &make_candidate(Tile::M1));
    assert!(
        break_taatsu < cut_float,
        "ターツ壊し({break_taatsu}) < 浮き牌切り({cut_float}) のはず"
    );
    assert_eq!(cut_float, 0.0);
}

#[test]
fn test_route_lock_follows_seven_pairs_route() {
    // 七対子路線では、一般形へドリフトする打牌（対子壊し）にペナルティを与える。
    // 対子壊しによる向聴数自体の悪化は基礎スコアが罰するため、
    // ここでは一般形のバックアップがある手（向聴数が変わらない打牌）で
    // ルートロックの差分が出ることを確認する。
    let mut state = CpuGameState::new();
    // 硬い対子4つ + 完成面子 + ターツ → 七対子・一般形とも2向聴
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

    // 対子壊しは総合向聴数こそ保つ（一般形2向聴のまま）が、
    // 七対子からは遠ざかる → ペナルティ
    let break_pair = route_lock_bonus(&ctx, &make_candidate(Tile::Z1));
    // 浮き牌切りは両形ともロスなし
    let cut_float = route_lock_bonus(&ctx, &make_candidate(Tile::M2));
    assert!(
        break_pair < cut_float,
        "対子壊し({break_pair}) < 浮き牌切り({cut_float}) のはず"
    );
    assert_eq!(cut_float, 0.0);
}

#[test]
fn test_toitoi_prospect_by_blocks() {
    // #157: 中以上では副露+対子・刻子が4ブロック以上で対々和候補
    let seat = Wind::East;
    let prev = Wind::East;

    // 3ブロック（副露2 + 対子1）+ 浮き牌多数 → 見込みなし
    let hand = tiles(&[Tile::P1, Tile::P1, Tile::M2, Tile::S3, Tile::M6, Tile::P7]);
    let melds = vec![pon_meld(Tile::M9), pon_meld(Tile::S9)];
    assert!(!has_yaku_prospect(&hand, &melds, seat, prev, true));

    // 4ブロック（副露2 + 対子2）→ 対々和の見込みあり
    let hand = tiles(&[Tile::P1, Tile::P1, Tile::S3, Tile::S3, Tile::M2, Tile::M6]);
    assert!(has_yaku_prospect(&hand, &melds, seat, prev, true));

    // 弱（従来ルール）: 浮き牌2種以下なら見込みあり
    assert!(has_yaku_prospect(&hand, &melds, seat, prev, false));
}

// --- 仕掛けの高度化（#164 #165）---

#[test]
fn test_tanyao_prospect_strict_conditions() {
    // #164: 中以上の喰いタン見込みは「么九牌2枚以下 + タンヤオ圏内に複数ブロック」
    let seat = Wind::East;
    let prev = Wind::East;
    let melds = vec![chi_meld(Tile::S2)]; // S234（中張牌のみ）

    // 么九牌3枚: 緩い条件では見込みあり、厳しい条件ではなし
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

    // 么九牌2枚 + タンヤオ圏内に2ブロック（M2M3, P4P5）→ 厳しい条件でも見込みあり
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

    // 么九牌なしでもタンヤオ圏内がバラバラ（ブロック1つ以下）なら見込みなし
    let hand = tiles(&[Tile::M2, Tile::M5, Tile::P5, Tile::S8]);
    assert!(!has_yaku_prospect(&hand, &melds, seat, prev, true));
}

#[test]
fn test_cheap_distant_call_detection() {
    // #165: 2向聴以上 + 打点要素なし + 子 → 安くて遠い仕掛け
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    let melds = vec![chi_meld(Tile::S2)];
    // 3色バラバラの2向聴超の手（ドラ・役牌なし）
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

    // 親なら例外
    let mut dealer_state = CpuGameState::new();
    dealer_state.my_seat_wind = Wind::East;
    assert!(!is_cheap_distant_call(&dealer_state, &hand, &melds, false));

    // ドラがあれば打点要素あり
    let mut dora_state = CpuGameState::new();
    dora_state.my_seat_wind = Wind::South;
    dora_state.dora_indicators = vec![Tile::new(Tile::M1)]; // ドラは M2（手牌にある）
    assert!(!is_cheap_distant_call(&dora_state, &hand, &melds, false));

    // 役牌対子があれば打点要素あり
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
    // 鳴いて1向聴以内に入る仕掛けは「遠い」扱いしない
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    let melds = vec![chi_meld(Tile::S2)];
    // 2面子 + 対子 + ターツ: チー後1向聴相当
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

// --- 国士無双ルート（#158 #159 #160 #161）---

/// 么九牌 n 種 + 中張牌の埋め草で13枚の手牌を作る
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
    // #160: 么九牌10種以上は国士無双を本線にする
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(10);
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);
}

#[test]
fn test_preferred_form_kokushi_nine_kinds_when_closer() {
    // #158: 9種は他形より明確に近いとき国士無双を採用する
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(9);
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);
}

#[test]
fn test_preferred_form_normal_with_seven_kinds_and_decent_hand() {
    // #158: 7種でも通常手に見込みがあるなら国士無双に向かわない
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
    // #161: 未所持の必要牌が4枚見えたら国士無双は成立しない
    let mut state = CpuGameState::new();
    state.my_hand = orphan_hand(10); // Z5/Z6/Z7 を持っていない
    state.all_discards[1] = vec![Tile::new(Tile::Z5); 4];
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_kokushi_route_abandoned_when_needed_tiles_thin_late() {
    // #161: 中盤以降、未所持の必要牌が残り1枚以下の種類が2つ以上なら見切る
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

    // 序盤（1巡目）はまだ見切らない
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);

    // 7巡目以降は見切る
    state.all_discards[0] = vec![Tile::new(Tile::P5); 6];
    assert_eq!(preferred_form(&state), Form::Normal);
}

#[test]
fn test_is_far_behind() {
    // トップと16000点差以上
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::East;
    state.scores = [8000, 42000, 25000, 25000];
    assert!(is_far_behind(&state));

    // 平場
    state.scores = [25000; 4];
    assert!(!is_far_behind(&state));

    // ラス目でも僅差なら対象外
    state.scores = [20000, 26000, 27000, 27000];
    assert!(!is_far_behind(&state));

    // ラス目で8000点以上離されている
    state.scores = [17000, 27000, 28000, 28000];
    assert!(is_far_behind(&state));
}

#[test]
fn test_preferred_form_kokushi_seven_kinds_when_far_behind() {
    // #159: 大きく負けているなら7種から、多少遠くても国士無双を狙う。
    // 通常形に見込みがある手で点棒状況だけを変えて比較する
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

    // 平場では狙わない（国士は通常形より遠い）
    state.scores = [25000; 4];
    assert_ne!(preferred_form(&state), Form::ThirteenOrphans);

    // 大差のラス目なら狙う
    state.scores = [5000, 45000, 25000, 25000];
    assert_eq!(preferred_form(&state), Form::ThirteenOrphans);
}

// --- 押し引き（#178）・まわし打ち（#179）---

#[test]
fn test_judge_push_folds_cheap_bad_shape_tenpai() {
    // #178: 愚形安手の聴牌は脅威がいれば降りる
    let mut state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    state.my_seat_wind = Wind::South; // 親の例外を避ける
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Fold);

    // 弱レベルは対象外
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);
}

#[test]
fn test_judge_push_pushes_good_shape_tenpai() {
    // #178: 良形聴牌は安手でも1人リーチには押す
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
    // #178: 満貫級の良形聴牌は2人リーチでも押す
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)]; // ドラ3
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
    // #178: 親は良形聴牌なら2人リーチでも押す（連荘価値）
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
    // #178: 2向聴でも満貫級（ドラ3相当）なら単独の脅威には押す
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
    state.dora_indicators = vec![Tile::new(Tile::M4), Tile::new(Tile::M4)]; // ドラ M5×2重
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);

    // ドラなしの安手2向聴は対象外（従来の撤退判断に委ねる）
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
    // #179: 強レベルは聴牌・1向聴で降りるとき安全度の重みを下げる
    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;

    // 聴牌形の手牌（山はまだ十分残っている）
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.remaining_tiles = 40;

    // 強レベル + 聴牌 → 重み150（まわし打ち）
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 150.0);

    // 中レベルは常にベタオリ（重み300）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 300.0);

    // 強レベルでも手が遠ければベタオリ（重み300）
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
    // #184/#185: 流局間際の聴牌・1向聴は形式聴牌を狙って重みを下げる
    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;

    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.scores = [26000, 25000, 25000, 24000]; // 自分(南)は2着
    state.remaining_tiles = 6;
    state.round_number = 1;
    state.total_rounds = 4;

    // #184（中以上）: 流局間際 → 重み150
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 150.0);

    // #185（強以上）: オーラスでトップ目でない → さらに聴牌維持重視（重み120）
    state.round_number = 3;
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let defending = DiscardContext {
        state: &state,
        config: &config,
        attacking: false,
    };
    assert_eq!(defense_safety_bonus(&defending, &candidate), 120.0);

    // #185: 親番でも聴牌維持重視
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

// --- 終盤処理・点棒状況（#183〜#191）---

#[test]
fn test_last_discard_safety_bonus() {
    // #186: 山が空の最終打牌は、脅威がいれば攻撃中でも安全度を重視する
    let mut candidate = make_candidate(Tile::M5);
    candidate.safety = 1.0;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // 山が残っていれば補正なし
    let mut state = CpuGameState::new();
    state.remaining_tiles = 10;
    state.player_riichi[1] = true;
    let ctx = attack_ctx(&state, &config);
    assert_eq!(last_discard_safety_bonus(&ctx, &candidate), 0.0);

    // 山が空 + リーチ者あり → 攻撃中でも補正
    state.remaining_tiles = 0;
    let ctx = attack_ctx(&state, &config);
    assert_eq!(last_discard_safety_bonus(&ctx, &candidate), 200.0);

    // 脅威がいなければ補正なし
    let mut state = CpuGameState::new();
    state.remaining_tiles = 0;
    let ctx = attack_ctx(&state, &config);
    assert_eq!(last_discard_safety_bonus(&ctx, &candidate), 0.0);
}

#[test]
fn test_cheap_call_allowed_when_final_round_speed_matters() {
    // #187: オーラスで安手和了でも順位が上がるなら速度優先（抑制解除）
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.scores = [25000, 24000, 26000, 25000]; // 自分(南)は2000点差の3着
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
    // 通常なら安くて遠い仕掛けだが、オーラスの僅差なので許容
    assert!(!is_cheap_distant_call(&state, &hand, &melds, false));

    // オーラスでなければ抑制される
    state.round_number = 1;
    assert!(is_cheap_distant_call(&state, &hand, &melds, false));
}

#[test]
fn test_cheap_call_suppressed_when_mangan_needed() {
    // #187: オーラスで満貫級が必要なら、近い（1向聴）仕掛けでも安手は控える
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.scores = [25000, 15000, 35000, 25000]; // 自分(南)はトップと2万点差
    state.round_number = 3;
    state.total_rounds = 4;
    let melds = vec![chi_meld(Tile::S2)];
    // チー後1向聴相当の手（2面子 + 対子 + ターツ）
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

    // 平場なら1向聴の仕掛けは抑制されない
    state.round_number = 1;
    assert!(!is_cheap_distant_call(&state, &hand, &melds, false));
}

#[test]
fn test_cheap_call_allowed_with_large_stakes() {
    // #191（強以上）: 供託・本場が大きければ安手仕掛けも許容
    let mut state = CpuGameState::new();
    state.my_seat_wind = Wind::South;
    state.riichi_sticks = 2; // 供託2000点
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
    // 強（供託考慮あり）: 許容
    assert!(!is_cheap_distant_call(&state, &hand, &melds, true));
    // 中（供託考慮なし）: 従来どおり抑制
    assert!(is_cheap_distant_call(&state, &hand, &melds, false));
}

#[test]
fn test_judge_push_top_in_second_half_folds() {
    // #188: 後半のトップ目は安手の良形聴牌でも降りる
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.scores = [25000, 40000, 20000, 15000]; // 自分(南)が大差トップ
    state.round_number = 3;
    state.total_rounds = 4;
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Fold);

    // 満貫級の良形聴牌だけは押す
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)]; // ドラ3
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);

    // 前半なら通常の判断（良形 + 脅威1人 → 押す）
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
    // #189: 大きく負けているときは2向聴の押し基準を下げる
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
    state.dora_indicators = vec![Tile::new(Tile::M4)]; // ドラ M5×2 = 推定4点相当
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // 平場: 基準（6.0）未満 → Neutral
    state.scores = [25000; 4];
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);

    // 大差のラス目: 基準が下がり押す
    state.scores = [42000, 8000, 25000, 25000];
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Push);
}

#[test]
fn test_judge_push_dealer_keeps_pushing_cheap_tenpai() {
    // #190: 親は愚形安手聴牌でも単独脅威には降り推奨しない（連荘価値）
    let mut state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    state.my_seat_wind = Wind::East;
    state.player_riichi[2] = true;
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    // 子なら Fold（既存テスト）、親なら Neutral（従来判断 = 押す）
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Neutral);
}

#[test]
fn test_judge_push_stakes_keep_cheap_tenpai_alive() {
    // #191（強以上）: 供託・本場が大きければ愚形安手聴牌でも降り推奨しない
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

    // 中レベルは供託を考慮しない → Fold
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_push(&ctx, 1), PushJudgement::Fold);
}

#[test]
fn test_judge_riichi_far_behind_declares_over_damaten() {
    // #189: 大きく負けているときは満貫確定でもダマにせずリーチで打点を伸ばす
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    state.my_seat_wind = Wind::South;
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)]; // ドラ3
    state.scores = [42000, 8000, 25000, 25000]; // 自分(南)は大差ラス
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

// --- リーチ・ダマ判断（#168〜#172）---

/// 聴牌済みの13枚 + ツモ切り対象の浮き牌で判定用の状態を作る
fn riichi_state(hand: &[u32], drawn: u32) -> CpuGameState {
    let mut state = CpuGameState::new();
    state.my_hand = tiles(hand);
    state.my_drawn = Some(Tile::new(drawn));
    state
}

/// 役なし聴牌（M8カンチャン待ち、ピンフ・タンヤオなし）
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

/// タンヤオ・ピンフ確定の両面聴牌（M3/M6待ち）
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

/// タンヤオのみのカンチャン聴牌（M7待ち）
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
    // #168: 役なし聴牌はリーチしないと和了できない → 宣言
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
    // #170: 全ての待ちでダマ満貫（タンヤオ+ピンフ+ドラ3）→ ダマ
    let mut state = riichi_state(&GOOD_SHAPE_TENPAI, Tile::Z3);
    // ドラ: S8×2（表示牌S7）+ M4×1（表示牌M3）= 3枚
    state.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)];

    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Damaten);

    // 弱レベルは #170 対象外 → 良形先制として宣言（#169）
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);
}

#[test]
fn test_judge_riichi_declares_good_shape() {
    // #169: 安手でも先制の良形聴牌はリーチ
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
    // #171: 愚形安手は早巡の先制なら宣言、中終盤はダマ
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);

    // 早巡（1巡目）→ 宣言
    let state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Declare);

    // 中終盤（11巡目）→ ダマ
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
    // #172: 強レベルは序盤の愚形を、良形変化が多ければ一巡待つ
    let state = riichi_state(&CHEAP_KANCHAN_TENPAI, Tile::Z4);

    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_riichi(&ctx, None), RiichiJudgement::Damaten);

    // 中レベルは #172 対象外 → 早巡先制の宣言（#171）
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
    // タンヤオ+ピンフ+ドラ3 = 5翻
    let state = {
        let mut s = CpuGameState::new();
        s.dora_indicators = vec![Tile::new(Tile::S7), Tile::new(Tile::M3)];
        s
    };
    let remaining = tiles(&GOOD_SHAPE_TENPAI);
    let han = estimate_ron_han(&state, &remaining, &[], Tile::M3);
    assert_eq!(han, Some(5));

    // 役なし → None
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
    // 白の対子があれば役牌の見込みあり
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
    // 副露も手牌も中張牌中心なら断么九の見込みあり
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
    // 萬子+字牌のみなら混一色の見込みあり
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
    // 副露が全て刻子で手牌が対子中心なら対々和の見込みあり
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
    // 3色バラバラ + 么九牌副露 + 役牌なし → 見込みなし
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
    // 3面子完成 + M9対子: M9ポンは向聴数を下げるが役の見込みがない
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
    // 既に3副露 → 裸単騎になるポンは禁止
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
    // 白対子のポンは推奨
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
    // 中張牌中心の手の中張牌ポンは中立（性格判断に委ねる）
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
    // 定石無効なら3副露でも Neutral（従来挙動の維持）
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
    // バラバラの3色手で么九牌絡みのチーは役の見込みがない
    // 手牌: M789 + P345 + S456 + M9M9 + S2 S7 → M7M8 で M9 をチー
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
    // M9チー（M7M8使用）: 副露に么九牌 → タンヤオ消滅、役牌なし、3色 → Forbid
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
    // 中張牌のみのチーで断么九の見込みが残る → Neutral
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

/// 暗カンすると手が壊れる手牌（S5×4を順子+刻子に使っている聴牌形）
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
    // S5×4 を S456 + S555 に使っている聴牌形: カンすると1向聴に落ちる
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
    // 弱レベルは対象外（初心者らしい雑なカンを許容）
    let state = hand_breaking_kan_state();
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_ankan(&ctx, Tile::S5), CallJudgement::Neutral);
}

/// カンしても向聴数が変わらない1向聴の手牌（P2×4が浮き刻子+1枚）
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
    // 他家リーチ中、カン後も聴牌でない → 新ドラリスクを取らない
    let mut state = shanten_keeping_kan_state();
    state.player_riichi[2] = true; // 西家がリーチ（自分は東家）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let ctx = CallContext {
        state: &state,
        config: &config,
    };
    assert_eq!(judge_ankan(&ctx, Tile::P2), CallJudgement::Forbid);
}

#[test]
fn test_heuristic_can_reference_candidate_and_context() {
    // 候補とコンテキストの両方を参照する定石が書けることを確認する
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
