//! 定石（heuristics）フレームワーク
//!
//! 人間らしい打牌判断の定石を「打牌候補へのスコア補正」として表現し、
//! CPU の強さレベルに応じて有効な定石だけを適用する（issue #142）。
//!
//! 個々の定石は `DiscardHeuristic` として定義し、`DISCARD_HEURISTICS` に
//! 登録する。定石をハードコードの分岐で書かないことで、
//! レベルごとの有効/無効切り替えと定石単位のテストを可能にする。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora};

use super::client::{CpuConfig, CpuLevel, is_yakuhai};
use super::evaluator::{DiscardCandidate, get_yakuhai_types};
use super::state::CpuGameState;

/// 打牌補正を計算する際の局面コンテキスト
pub struct DiscardContext<'a> {
    /// CPU が観測しているゲーム状態
    pub state: &'a CpuGameState,
    /// CPU 設定
    pub config: &'a CpuConfig,
    /// 攻撃継続中か（false なら守備優先）
    pub attacking: bool,
}

/// 定石1つの定義
///
/// `apply` は「この牌を捨てる」ことの良さに対する補正値を返す。
/// 正の値はその牌を切りやすく、負の値は切りにくくする。
/// スケールは `select_best_discard` の基本スコア（向聴数1段階 = 100.0）に合わせる。
pub struct DiscardHeuristic {
    /// 定石名（ログ・テスト用）
    pub name: &'static str,
    /// この定石が有効になる最低レベル
    pub min_level: CpuLevel,
    /// 補正関数
    pub apply: fn(&DiscardContext, &DiscardCandidate) -> f64,
}

/// 打牌定石のレジストリ
///
/// issue #142 の定石を後続の変更でここに追加していく。
/// 補正は合算されるため、登録順は結果に影響しない。
pub const DISCARD_HEURISTICS: &[DiscardHeuristic] = &[
    // #147: 孤立字牌・孤立么九牌から優先して切る（弱以上）
    DiscardHeuristic {
        name: "isolated-honor-terminal-first",
        min_level: CpuLevel::Weak,
        apply: isolated_tile_bonus,
    },
    // #148: 両面ターツを辺張・嵌張より優先する（弱以上）
    DiscardHeuristic {
        name: "protect-ryanmen-shapes",
        min_level: CpuLevel::Weak,
        apply: shape_protection_bonus,
    },
    // #152: ドラを雑に切らない（弱以上）
    DiscardHeuristic {
        name: "protect-dora",
        min_level: CpuLevel::Weak,
        apply: dora_protection_bonus,
    },
    // #173/#174/#176: 守備時は現物を最優先で切る（弱以上）
    DiscardHeuristic {
        name: "genbutsu-first-when-defending",
        min_level: CpuLevel::Weak,
        apply: defense_safety_bonus,
    },
];

// ============================================================================
// 打牌定石の実装
// ============================================================================

/// 打牌候補を除いた残り手牌の牌種カウントを返す
///
/// 「この牌を切ったときに残る形」を評価するため、候補牌1枚を差し引く。
fn remaining_counts(state: &CpuGameState, discard: Tile) -> [u8; 34] {
    let mut counts = [0u8; 34];
    for t in &state.my_hand {
        counts[t.get() as usize] += 1;
    }
    if let Some(d) = state.my_drawn {
        counts[d.get() as usize] += 1;
    }
    let idx = discard.get() as usize;
    counts[idx] = counts[idx].saturating_sub(1);
    counts
}

/// #147: 孤立した字牌・么九牌は切りやすくする
///
/// 序盤のバラバラな手では、孤立した客風牌 > 1・9牌 > 役牌 > 2・8牌
/// の順で打牌候補としての価値を上げる（=手牌としての価値を下げる）。
/// 役牌は対子になれば役が付くため 1・9 牌より残す。
/// 中張牌の孤立牌には補正を与えず、相対的に残りやすくする。
fn isolated_tile_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    let counts = remaining_counts(ctx.state, c.tile);
    let tt = c.tile.get();

    // 対子・刻子の一部なら孤立牌ではない
    if counts[tt as usize] >= 1 {
        return 0.0;
    }

    if tt >= 27 {
        // 字牌: 役牌は対子になれば役があるため、1・9牌よりさらに残す
        if is_yakuhai(tt, ctx.state.my_seat_wind, ctx.state.prevailing_wind) {
            8.0
        } else {
            16.0
        }
    } else {
        // 数牌: 前後2つ以内に牌があればターツ候補なので孤立ではない
        let pos = (tt % 9) as i32;
        let suit_start = tt - tt % 9;
        let near = |offset: i32| -> bool {
            let q = pos + offset;
            (0..9).contains(&q) && counts[(suit_start + q as TileType) as usize] > 0
        };
        if near(-2) || near(-1) || near(1) || near(2) {
            return 0.0;
        }
        match pos {
            0 | 8 => 10.0, // 1, 9
            1 | 7 => 5.0,  // 2, 8
            _ => 0.0,      // 中張牌は雑に切らない
        }
    }
}

/// #148: 両面ターツの牌は残し、辺張・嵌張の牌は整理しやすくする
fn shape_protection_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    // 守備時は安全度を優先する
    if !ctx.attacking {
        return 0.0;
    }

    let tt = c.tile.get();
    if tt >= 27 {
        return 0.0; // 字牌にターツはない
    }

    let counts = remaining_counts(ctx.state, c.tile);
    if counts[tt as usize] >= 1 {
        return 0.0; // 対子・刻子側の判断はしない
    }

    let pos = (tt % 9) as i32;
    let suit_start = tt - tt % 9;
    let has = |offset: i32| -> bool {
        let q = pos + offset;
        (0..9).contains(&q) && counts[(suit_start + q as TileType) as usize] > 0
    };

    let lower = has(-1);
    let upper = has(1);

    if lower && upper {
        return 0.0; // 順子の真ん中（切れば向聴数で評価される）
    }

    if lower || upper {
        // 隣の牌と2枚ターツを構成している
        // ターツの下端位置で両面か辺張かを判定（0-indexed: 1..=6 始まりが両面）
        let pair_low = if lower { pos - 1 } else { pos };
        let two_sided = (1..=6).contains(&pair_low);
        return if two_sided {
            -6.0 // 両面ターツの牌は守る
        } else {
            3.0 // 辺張（12/89）は整理しやすく
        };
    }

    if has(-2) || has(2) {
        return 3.0; // 嵌張も愚形として整理しやすく
    }

    0.0
}

/// #152: ドラ・赤ドラを雑に切らない
///
/// 攻撃中はドラ1枚につきペナルティを与えて手に残しやすくする。
/// 守備時は補正しない（安全度を優先する）。
fn dora_protection_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let mut dora_count = u32::from(c.tile.is_red_dora());
    for indicator in &ctx.state.dora_indicators {
        if dora_indicator_to_dora(indicator.get()) == c.tile.get() {
            dora_count += 1;
        }
    }
    -(dora_count as f64) * 12.0
}

/// #173/#174/#176: 守備時は安全度を最優先する（ベタオリ）
///
/// 安全度に大きな重みを掛けることで、現物 > スジ・字牌 > 無筋么九牌 >
/// 無筋中張牌（456が最も危険）の順で打牌が選ばれる。
/// 重み300は向聴数3段階分に相当し、聴牌を崩してでも現物を切る。
fn defense_safety_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if ctx.attacking {
        return 0.0;
    }
    c.safety * 300.0
}

/// 打牌候補1つに対して、有効な全定石の補正値を合算する
pub fn discard_adjustment(ctx: &DiscardContext, candidate: &DiscardCandidate) -> f64 {
    discard_adjustment_with(DISCARD_HEURISTICS, ctx, candidate)
}

/// レジストリを指定して補正値を合算する
///
/// レベルが `min_level` 未満の定石は適用しない。
/// `heuristics_enabled` が false の場合は全定石を無効化する（新旧比較用）。
fn discard_adjustment_with(
    heuristics: &[DiscardHeuristic],
    ctx: &DiscardContext,
    candidate: &DiscardCandidate,
) -> f64 {
    if !ctx.config.heuristics_enabled {
        return 0.0;
    }
    heuristics
        .iter()
        .filter(|h| ctx.config.level >= h.min_level)
        .map(|h| (h.apply)(ctx, candidate))
        .sum()
}

// ============================================================================
// 鳴き判断の定石
//
// 打牌定石（スコア補正の合算）と異なり、鳴きは個別の意思決定なので
// 「禁止 / 推奨 / 中立」の三値判定で表現する。禁止は推奨より優先される。
// ============================================================================

/// 鳴き判断の文脈
pub struct CallContext<'a> {
    /// CPU が観測しているゲーム状態
    pub state: &'a CpuGameState,
    /// CPU 設定
    pub config: &'a CpuConfig,
}

/// 鳴き・カンに対する定石の判定結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallJudgement {
    /// 定石上、鳴くべきでない
    Forbid,
    /// 定石上、積極的に鳴くべき（性格・積極度の判断を上書きする）
    Encourage,
    /// 定石は関与しない（既存の性格・積極度判断に委ねる）
    Neutral,
}

/// ポンに対する定石判定
///
/// 適用される定石:
/// - 裸単騎回避（#166, 弱以上）: 4副露目になる鳴きはしない
/// - 役なし鳴き禁止（#162, 弱以上）: 鳴いた後に役の見込みがなければ鳴かない
/// - 役牌対子は早めにポン（#163, 弱以上）: 役牌のポンは性格によらず推奨
///
/// 呼び出し元で「向聴数が下がること」は確認済みである前提。
pub fn judge_pon(ctx: &CallContext, called_tile: Tile) -> CallJudgement {
    if !ctx.config.heuristics_enabled {
        return CallJudgement::Neutral;
    }

    // 裸単騎回避（弱以上）
    if ctx.state.my_melds().len() >= 3 {
        return CallJudgement::Forbid;
    }

    // 役なし鳴き禁止（弱以上）
    if let Some((hand_after, melds_after)) = hand_after_pon(ctx.state, called_tile)
        && !has_yaku_prospect(
            &hand_after,
            &melds_after,
            ctx.state.my_seat_wind,
            ctx.state.prevailing_wind,
        )
    {
        return CallJudgement::Forbid;
    }

    // 役牌対子は早めにポン（弱以上）
    if is_yakuhai(
        called_tile.get(),
        ctx.state.my_seat_wind,
        ctx.state.prevailing_wind,
    ) {
        return CallJudgement::Encourage;
    }

    CallJudgement::Neutral
}

/// チーに対する定石判定
///
/// 適用される定石:
/// - 裸単騎回避（#166, 弱以上）
/// - 役なし鳴き禁止（#162, 弱以上）
///
/// 呼び出し元で「向聴数が下がること」は確認済みである前提。
pub fn judge_chi(ctx: &CallContext, called_tile: Tile, hand_tiles: [Tile; 2]) -> CallJudgement {
    if !ctx.config.heuristics_enabled {
        return CallJudgement::Neutral;
    }

    // 裸単騎回避（弱以上）
    if ctx.state.my_melds().len() >= 3 {
        return CallJudgement::Forbid;
    }

    // 役なし鳴き禁止（弱以上）
    if let Some((hand_after, melds_after)) = hand_after_chi(ctx.state, called_tile, hand_tiles)
        && !has_yaku_prospect(
            &hand_after,
            &melds_after,
            ctx.state.my_seat_wind,
            ctx.state.prevailing_wind,
        )
    {
        return CallJudgement::Forbid;
    }

    CallJudgement::Neutral
}

/// 暗カンに対する定石判定（#167, 中以上）
///
/// - 向聴数が悪化するカン（手を壊すカン）はしない
/// - 他家リーチ中は、カン後も聴牌している場合を除きカンしない
///   （新ドラで相手の打点を上げるリスクを評価する）
pub fn judge_ankan(ctx: &CallContext, tile_type: TileType) -> CallJudgement {
    if !ctx.config.heuristics_enabled || ctx.config.level < CpuLevel::Normal {
        return CallJudgement::Neutral;
    }

    let mut all_tiles = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        all_tiles.push(drawn);
    }

    let melds = ctx.state.my_melds_for_analysis();

    // カン前の向聴数
    let before_hand = Hand::new_with_melds(all_tiles.clone(), melds.clone(), None);
    let before = calc_shanten_number(&before_hand);

    // カン後の向聴数（対象の4枚を除き、カンを面子として加える）
    let remaining: Vec<Tile> = all_tiles
        .iter()
        .filter(|t| t.get() != tile_type)
        .copied()
        .collect();
    let mut melds_after = melds;
    melds_after.push(Meld {
        tiles: vec![Tile::new(tile_type); 3],
        category: MeldType::Kan,
        from: MeldFrom::Myself,
        called_tile: None,
    });
    let after_hand = Hand::new_with_melds(remaining, melds_after, None);
    let after = calc_shanten_number(&after_hand);

    // 手を壊すカンはしない
    if after > before {
        return CallJudgement::Forbid;
    }

    // 他家リーチ中は聴牌維持できる場合のみカンする
    let my_idx = CpuGameState::wind_to_index(ctx.state.my_seat_wind);
    let opponent_riichi = ctx
        .state
        .player_riichi
        .iter()
        .enumerate()
        .any(|(i, &r)| i != my_idx && r);
    if opponent_riichi && !after.is_ready_or_won() {
        return CallJudgement::Forbid;
    }

    CallJudgement::Neutral
}

/// ポンした後の手牌と副露を構築する
///
/// 手牌に同種の牌が2枚なければ `None`。
fn hand_after_pon(state: &CpuGameState, called_tile: Tile) -> Option<(Vec<Tile>, Vec<Meld>)> {
    let tt = called_tile.get();
    let mut remaining = state.my_hand.clone();
    let mut removed = 0;
    remaining.retain(|t| {
        if t.get() == tt && removed < 2 {
            removed += 1;
            false
        } else {
            true
        }
    });
    if removed < 2 {
        return None;
    }

    let mut melds = state.my_melds_for_analysis();
    melds.push(Meld {
        tiles: vec![called_tile, called_tile, called_tile],
        category: MeldType::Pon,
        from: MeldFrom::Unknown,
        called_tile: Some(called_tile),
    });
    Some((remaining, melds))
}

/// チーした後の手牌と副露を構築する
///
/// 指定の2枚が手牌になければ `None`。
fn hand_after_chi(
    state: &CpuGameState,
    called_tile: Tile,
    hand_tiles: [Tile; 2],
) -> Option<(Vec<Tile>, Vec<Meld>)> {
    let mut remaining = state.my_hand.clone();
    let mut chi_tiles = Vec::new();
    for &target in &hand_tiles {
        let pos = remaining.iter().position(|t| *t == target)?;
        chi_tiles.push(remaining.remove(pos));
    }

    let mut melds = state.my_melds_for_analysis();
    melds.push(Meld {
        tiles: vec![called_tile, chi_tiles[0], chi_tiles[1]],
        category: MeldType::Chi,
        from: MeldFrom::Previous,
        called_tile: Some(called_tile),
    });
    Some((remaining, melds))
}

/// 鳴いた後の手に和了役の見込みがあるか（簡易判定）
///
/// 副露した手で成立しうる代表的な役の見込みを判定する（#162）:
/// - 役牌: 手牌+副露に役牌が2枚以上ある
/// - 断么九: 副露が全て中張牌で、手牌の么九牌が3枚以下（切って移行できる）
/// - 混一色/清一色: 数牌が1色に収まっている
/// - 対々和: 副露が全て刻子系で、手牌の浮き牌が2種以下
///
/// チャンタ系などの稀な役は考慮しない（見込みなしと誤判定しても
/// 「鳴かない」側に倒れるだけで安全）。
pub fn has_yaku_prospect(
    hand_tiles: &[Tile],
    melds: &[Meld],
    seat_wind: Wind,
    prevailing_wind: Wind,
) -> bool {
    // 手牌 + 副露の牌種ごとの枚数
    let mut counts = [0u8; 34];
    for t in hand_tiles {
        counts[t.get() as usize] += 1;
    }
    for meld in melds {
        for t in &meld.tiles {
            counts[t.get() as usize] += 1;
        }
    }

    // 役牌: 対子以上があれば刻子にできる見込みがある
    for yh in get_yakuhai_types(seat_wind, prevailing_wind) {
        if counts[yh as usize] >= 2 {
            return true;
        }
    }

    // 断么九: 副露が全て中張牌で、手牌の么九牌が少ない
    let melds_all_simple = melds
        .iter()
        .all(|m| m.tiles.iter().all(|t| !t.is_1_9_honor()));
    if melds_all_simple {
        let terminal_honor_count = hand_tiles.iter().filter(|t| t.is_1_9_honor()).count();
        if terminal_honor_count <= 3 {
            return true;
        }
    }

    // 混一色/清一色/字一色: 数牌が1色以下に収まっている
    let mut suits_used = [false; 3];
    for (tile_type, &count) in counts.iter().enumerate().take(27) {
        if count > 0 {
            suits_used[tile_type / 9] = true;
        }
    }
    if suits_used.iter().filter(|&&u| u).count() <= 1 {
        return true;
    }

    // 対々和: 副露が全て刻子系で、手牌が対子・刻子中心
    let melds_all_triplets = melds
        .iter()
        .all(|m| matches!(m.category, MeldType::Pon | MeldType::Kan | MeldType::Kakan));
    if melds_all_triplets && !melds.is_empty() {
        let hand_singles = {
            let mut hand_counts = [0u8; 34];
            for t in hand_tiles {
                hand_counts[t.get() as usize] += 1;
            }
            hand_counts.iter().filter(|&&c| c == 1).count()
        };
        if hand_singles <= 2 {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
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
        let config =
            CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
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
        let config =
            CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
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
        state.prevailing_wind = Wind::East;
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
        assert!(has_yaku_prospect(&hand, &melds, Wind::East, Wind::East));
    }

    #[test]
    fn test_yaku_prospect_tanyao() {
        // 副露も手牌も中張牌中心なら断么九の見込みあり
        let hand = tiles(&[Tile::M2, Tile::M3, Tile::P4, Tile::P5, Tile::S6, Tile::M9]);
        let melds = vec![chi_meld(Tile::S2)];
        assert!(has_yaku_prospect(&hand, &melds, Wind::East, Wind::East));
    }

    #[test]
    fn test_yaku_prospect_honitsu() {
        // 萬子+字牌のみなら混一色の見込みあり
        let hand = tiles(&[Tile::M1, Tile::M2, Tile::M3, Tile::M7, Tile::Z2, Tile::Z3]);
        let melds = vec![chi_meld(Tile::M4)];
        assert!(has_yaku_prospect(&hand, &melds, Wind::East, Wind::East));
    }

    #[test]
    fn test_yaku_prospect_toitoi() {
        // 副露が全て刻子で手牌が対子中心なら対々和の見込みあり
        let hand = tiles(&[Tile::M9, Tile::M9, Tile::P1, Tile::P1, Tile::S9]);
        let melds = vec![pon_meld(Tile::M1), pon_meld(Tile::S1)];
        assert!(has_yaku_prospect(&hand, &melds, Wind::East, Wind::East));
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
        assert!(!has_yaku_prospect(&hand, &melds, Wind::East, Wind::East));
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
        let config =
            CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics();
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
            "honor-in-defense",
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

        let honor = make_candidate(Tile::Z1);
        let number = make_candidate(Tile::M5);

        let defending = DiscardContext {
            state: &state,
            config: &config,
            attacking: false,
        };
        assert_eq!(
            discard_adjustment_with(&heuristics, &defending, &honor),
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
            discard_adjustment_with(&heuristics, &attacking, &honor),
            0.0
        );
    }
}
