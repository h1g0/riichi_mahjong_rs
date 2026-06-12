//! 定石（heuristics）フレームワーク
//!
//! 人間らしい打牌判断の定石を「打牌候補へのスコア補正」として表現し、
//! CPU の強さレベルに応じて有効な定石だけを適用する（issue #142）。
//!
//! 個々の定石は `DiscardHeuristic` として定義し、`DISCARD_HEURISTICS` に
//! 登録する。定石をハードコードの分岐で書かないことで、
//! レベルごとの有効/無効切り替えと定石単位のテストを可能にする。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{
    HandAnalyzer, calc_shanten_number, calc_shanten_number_by_form,
};
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::hand_info::status::Status;
use mahjong_core::scoring::score::calculate_score;
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora};
use mahjong_core::winning_hand::name::Form;

use super::client::{CpuConfig, CpuLevel, is_yakuhai};
use super::defense::ORPHAN_TYPES;
use super::evaluator::{DiscardCandidate, estimate_hand_value, get_yakuhai_types};
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
    // #149: 一般形では5ブロックを意識する（中以上）
    DiscardHeuristic {
        name: "five-block-surplus",
        min_level: CpuLevel::Normal,
        apply: five_block_bonus,
    },
    // #151: 唯一の雀頭候補を安易に壊さない（中以上）
    DiscardHeuristic {
        name: "protect-sole-pair",
        min_level: CpuLevel::Normal,
        apply: sole_pair_protection,
    },
    // #153: 見えている枚数で有効牌を補正する（中以上）
    DiscardHeuristic {
        name: "dismantle-dead-shapes",
        min_level: CpuLevel::Normal,
        apply: dead_shape_bonus,
    },
    // #150: 3対子以上は原則としてほぐす（強以上）
    DiscardHeuristic {
        name: "break-excess-pairs",
        min_level: CpuLevel::Strong,
        apply: excess_pair_bonus,
    },
    // #154/#155/#156: 七対子と一般形の路線選択に沿って打牌する（中以上）
    DiscardHeuristic {
        name: "follow-hand-route",
        min_level: CpuLevel::Normal,
        apply: route_lock_bonus,
    },
    // #186: 河底牌（最終打牌）で危険牌を切らない（中以上）
    DiscardHeuristic {
        name: "safe-last-discard",
        min_level: CpuLevel::Normal,
        apply: last_discard_safety_bonus,
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
///
/// #179（強以上）: 聴牌・1向聴で降りる場合は重みを150に下げる。
/// スジ程度の安全度差（0.25 → 37.5点）では向聴数1段階（100点）を
/// 覆せなくなるため、現物で形を崩す代わりにスジ・字牌などの
/// 安全寄りの牌で聴牌復帰を狙う「まわし打ち」になる。
/// 無筋中張牌との差（0.85 → 127.5点）は依然として向聴数を上回るので、
/// 危険牌を押してまで形は守らない。
fn defense_safety_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if ctx.attacking {
        return 0.0;
    }

    let mut weight = 300.0;

    let mut all_tiles = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        all_tiles.push(drawn);
    }
    let hand = Hand::new_with_melds(all_tiles, ctx.state.my_melds_for_analysis(), None);
    let close = calc_shanten_number(&hand).as_i32() <= 1;

    if close {
        // #179（強以上）: まわし打ち
        if ctx.config.level >= CpuLevel::Strong {
            weight = 150.0;
        }
        // #184（中以上）: 流局間際は安全牌を切りながら形式聴牌を狙う
        if ctx.config.level >= CpuLevel::Normal && ctx.state.remaining_tiles <= 8 {
            weight = 150.0;
            // #185（強以上）: 親番・オーラスで順位が懸かる場面では
            // 聴牌維持の価値をさらに上げる
            if ctx.config.level >= CpuLevel::Strong
                && (ctx.state.my_seat_wind == Wind::East
                    || (ctx.state.is_final_round() && !ctx.state.is_top()))
            {
                weight = 120.0;
            }
        }
    }

    c.safety * weight
}

/// #186: 河底牌（最終打牌）で危険牌を切らない
///
/// 山が空のときの打牌はこの局の最後の行動であり、手を進める意味がない。
/// 攻撃中でも安全度に大きな重みを掛ける。形式聴牌の維持（約100点 =
/// 向聴数1段階）はスジ程度の安全差なら優先されるが、無筋の危険牌を
/// 押してまで維持はしない。
fn last_discard_safety_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if ctx.state.remaining_tiles > 0 {
        return 0.0;
    }

    // 脅威（リーチ者・3副露以上）がいなければ補正不要
    let my_idx = CpuGameState::wind_to_index(ctx.state.my_seat_wind);
    let any_threat = (0..4).any(|i| {
        i != my_idx && (ctx.state.player_riichi[i] || ctx.state.player_melds[i].len() >= 3)
    });
    if !any_threat {
        return 0.0;
    }

    c.safety * 200.0
}

/// 手牌全体（ツモ込み・副露込み）のブロック数を数える
///
/// ブロック = 面子（副露含む）+ 対子 + ターツ。
/// 和了形は4面子1雀頭 = 5ブロックなので、6以上は持ちすぎ。
///
/// `HandAnalyzer` の分解は向聴数計算に必要な5ブロックまでしか記録しない
/// （余剰ブロックは孤立牌扱いになる）ため、ここでは牌種カウントから
/// 貪欲に数える。刻子 → 順子 → 対子 → ターツの順に取り出す。
fn count_blocks(state: &CpuGameState) -> usize {
    let mut counts = [0u8; 34];
    for t in &state.my_hand {
        counts[t.get() as usize] += 1;
    }
    if let Some(drawn) = state.my_drawn {
        counts[drawn.get() as usize] += 1;
    }

    state.my_melds().len() + greedy_block_count(counts)
}

/// 牌種カウントからブロック数（面子+対子+ターツ）を貪欲に数える
///
/// 刻子 → 順子 → 対子 → ターツの順に取り出す。
fn greedy_block_count(mut counts: [u8; 34]) -> usize {
    let mut blocks = 0;

    // 刻子
    for c in counts.iter_mut() {
        if *c >= 3 {
            *c -= 3;
            blocks += 1;
        }
    }

    // 順子（数牌のみ、昇順に貪欲）
    for suit_start in [0usize, 9, 18] {
        for pos in 0..7 {
            let i = suit_start + pos;
            while counts[i] > 0 && counts[i + 1] > 0 && counts[i + 2] > 0 {
                counts[i] -= 1;
                counts[i + 1] -= 1;
                counts[i + 2] -= 1;
                blocks += 1;
            }
        }
    }

    // 対子
    for c in counts.iter_mut() {
        if *c >= 2 {
            *c -= 2;
            blocks += 1;
        }
    }

    // ターツ（隣接・嵌張。数牌のみ、昇順に貪欲）
    for suit_start in [0usize, 9, 18] {
        for pos in 0..8 {
            let i = suit_start + pos;
            if counts[i] > 0 && counts[i + 1] > 0 {
                counts[i] -= 1;
                counts[i + 1] -= 1;
                blocks += 1;
            } else if pos < 7 && counts[i] > 0 && counts[i + 2] > 0 {
                counts[i] -= 1;
                counts[i + 2] -= 1;
                blocks += 1;
            }
        }
    }

    blocks
}

/// 手牌中で「ちょうど2枚」ある牌種（対子）のリストを返す
///
/// 3枚以上は刻子（またはカン材）とみなして含めない。
fn pair_types(state: &CpuGameState) -> Vec<TileType> {
    let mut counts = [0u8; 34];
    for t in &state.my_hand {
        counts[t.get() as usize] += 1;
    }
    if let Some(d) = state.my_drawn {
        counts[d.get() as usize] += 1;
    }
    counts
        .iter()
        .enumerate()
        .filter(|&(_, &c)| c == 2)
        .map(|(i, _)| i as TileType)
        .collect()
}

/// #149: 6ブロック以上の手では弱いブロック（愚形ターツ・余剰対子）を整理する
fn five_block_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }
    if count_blocks(ctx.state) < 6 {
        return 0.0;
    }

    let tt = c.tile.get();
    let counts = remaining_counts(ctx.state, c.tile);

    // 余剰対子: 対子が2つ以上あれば、1つは整理してよい
    if counts[tt as usize] == 1 && pair_types(ctx.state).len() >= 2 {
        return 4.0;
    }

    // 愚形ターツ（辺張・嵌張）の構成牌
    if tt < 27 && counts[tt as usize] == 0 {
        let pos = (tt % 9) as i32;
        let suit_start = tt - tt % 9;
        let has = |offset: i32| -> bool {
            let q = pos + offset;
            (0..9).contains(&q) && counts[(suit_start + q as TileType) as usize] > 0
        };
        let lower = has(-1);
        let upper = has(1);
        if lower != upper {
            // 2枚ターツ: 辺張なら整理対象
            let pair_low = if lower { pos - 1 } else { pos };
            if !(1..=6).contains(&pair_low) {
                return 6.0;
            }
        } else if !lower && !upper && (has(-2) || has(2)) {
            // 嵌張も整理対象
            return 6.0;
        }
    }

    0.0
}

/// #151: 唯一の雀頭候補（対子が1つだけ）の牌は壊さない
fn sole_pair_protection(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let pairs = pair_types(ctx.state);
    if pairs.len() != 1 || pairs[0] != c.tile.get() {
        return 0.0;
    }

    // 刻子があれば雀頭候補は他にもある
    let mut counts = [0u8; 34];
    for t in &ctx.state.my_hand {
        counts[t.get() as usize] += 1;
    }
    if let Some(d) = ctx.state.my_drawn {
        counts[d.get() as usize] += 1;
    }
    if counts.iter().any(|&n| n >= 3) {
        return 0.0;
    }

    -12.0
}

/// #153: 受け牌がほぼ枯れている形（死にターツ）は整理する
///
/// 嵌張・辺張は待ち1種なので、残り1枚以下なら死にターツとして扱う。
/// 両面も両方の待ちが計2枚以下なら同様に扱う。
fn dead_shape_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let tt = c.tile.get();
    if tt >= 27 {
        return 0.0;
    }

    let counts = remaining_counts(ctx.state, c.tile);
    if counts[tt as usize] > 0 {
        return 0.0; // 対子・刻子側は対象外
    }

    let pos = (tt % 9) as i32;
    let suit_start = tt - tt % 9;
    let has = |offset: i32| -> bool {
        let q = pos + offset;
        (0..9).contains(&q) && counts[(suit_start + q as TileType) as usize] > 0
    };

    let visible = ctx.state.visible_tile_counts();
    let remaining_of = |p: i32| -> u32 {
        if (0..9).contains(&p) {
            4u32.saturating_sub(visible[(suit_start + p as TileType) as usize] as u32)
        } else {
            0
        }
    };

    let lower = has(-1);
    let upper = has(1);

    if lower && upper {
        return 0.0; // 順子の真ん中
    }

    if lower || upper {
        // 隣接2枚ターツ: 両面は両端、辺張は片端のみが待ち
        let pair_low = if lower { pos - 1 } else { pos };
        let waits = remaining_of(pair_low - 1) + remaining_of(pair_low + 2);
        if waits <= 1 {
            return 10.0;
        }
        if waits <= 2 {
            return 4.0;
        }
        return 0.0;
    }

    if has(-2) || has(2) {
        // 嵌張: 真ん中の1種のみが待ち
        let mid = if has(-2) { pos - 1 } else { pos + 1 };
        let waits = remaining_of(mid);
        if waits <= 1 {
            return 10.0;
        }
        if waits <= 2 {
            return 4.0;
        }
    }

    0.0
}

/// #150: 3対子以上は順子化しやすい対子からほぐす（強以上）
///
/// 一般形が七対子より明確に近い場合のみ適用する。
/// 中張牌の対子は残った1枚が両面候補になるため、ほぐす優先度が高い。
/// 字牌対子はポン材・雀頭として残す。
fn excess_pair_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let pairs = pair_types(ctx.state);
    if pairs.len() < 3 || !pairs.contains(&c.tile.get()) {
        return 0.0;
    }

    // 七対子の方が近い（または同等の）手はほぐさない
    let mut all_tiles = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        all_tiles.push(drawn);
    }
    let hand = Hand::new_with_melds(all_tiles, ctx.state.my_melds_for_analysis(), None);
    let normal = calc_shanten_number_by_form(&hand, Form::Normal);
    let seven_pairs = calc_shanten_number_by_form(&hand, Form::SevenPairs);
    if seven_pairs <= normal {
        return 0.0;
    }

    let tt = c.tile.get();
    if tt >= 27 {
        return 0.0; // 字牌対子は残す
    }
    match tt % 9 {
        0 | 8 => 3.0, // 1/9 の対子
        _ => 5.0,     // 中張牌の対子は順子材として残った1枚が活きる
    }
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

/// 七対子・国士無双・一般形のどれを本線にするかを判定する
/// （#154/#155/#156, #158/#159/#160/#161）
///
/// - 副露があれば一般形（七対子・国士無双は門前限定）
/// - 么九牌の種類数と点棒状況に応じて国士無双ルートを選ぶ（#158〜#161）
/// - 対子が4つ未満なら一般形（#154: 4トイツ未満で七対子を本線にしない）
/// - 対子が4つ以上でも、一般形の方が近い場合や、連続対子などの
///   複合形が多い場合は一般形を優先する（#155）
/// - 字牌・么九牌・孤立した数牌対子（横に伸びにくい対子）が過半数なら
///   七対子に向かう（#156）
pub(crate) fn preferred_form(state: &CpuGameState) -> Form {
    if !state.my_melds().is_empty() {
        return Form::Normal;
    }

    let mut all_tiles = state.my_hand.clone();
    if let Some(drawn) = state.my_drawn {
        all_tiles.push(drawn);
    }

    // 国士無双ルート（#158〜#161）
    if kokushi_route_viable(state, &all_tiles) {
        return Form::ThirteenOrphans;
    }

    let pairs = pair_types(state);
    if pairs.len() < 4 {
        return Form::Normal;
    }

    let hand = Hand::new(all_tiles.clone(), None);
    let seven_pairs = calc_shanten_number_by_form(&hand, Form::SevenPairs);
    let normal = calc_shanten_number_by_form(&hand, Form::Normal);
    if seven_pairs > normal {
        return Form::Normal;
    }

    // 対子の質: 横に伸びにくい対子が過半数なら七対子寄り
    let mut counts = [0u8; 34];
    for t in &all_tiles {
        counts[t.get() as usize] += 1;
    }
    let stiff = pairs
        .iter()
        .filter(|&&tt| is_stiff_pair(&counts, tt))
        .count();
    if stiff * 2 > pairs.len() {
        Form::SevenPairs
    } else {
        Form::Normal
    }
}

/// 国士無双を本線にすべきか（#158/#159/#160/#161）
///
/// - #160: 么九牌10種以上なら本線にする
/// - #158: 8〜9種は他形と同等以上に近ければ採用、
///   7種は通常手に見込みがない（5向聴以上）ときのみ候補に留める
/// - #159: 大きく負けている場合は7種から、多少遠くても狙う
///   （役満で逆転する価値がある）
/// - #161: 未所持の必要牌が枯れている（4枚見え）なら成立不可能なので狙わない。
///   中盤以降、未所持の必要牌が残り1枚以下の種類が2つ以上あれば見切る。
///   この判定は自分の意思決定なので、自分の手牌を含む全ての見え情報を使う。
fn kokushi_route_viable(state: &CpuGameState, all_tiles: &[Tile]) -> bool {
    let mut counts = [0u8; 34];
    for t in all_tiles {
        counts[t.get() as usize] += 1;
    }
    let kinds = ORPHAN_TYPES
        .iter()
        .filter(|&&t| counts[t as usize] > 0)
        .count();
    if kinds < 7 {
        return false;
    }

    // #161: 未所持の必要牌の枯れチェック
    let visible = state.visible_tile_counts();
    let missing_dead = ORPHAN_TYPES
        .iter()
        .any(|&t| counts[t as usize] == 0 && visible[t as usize] >= 4);
    if missing_dead {
        return false;
    }
    let thin_missing = ORPHAN_TYPES
        .iter()
        .filter(|&&t| counts[t as usize] == 0 && visible[t as usize] >= 3)
        .count();
    if state.turn() >= 7 && thin_missing >= 2 {
        return false;
    }

    // #160: 10種以上は本線
    if kinds >= 10 {
        return true;
    }

    let hand = Hand::new(all_tiles.to_vec(), None);
    let orphans = calc_shanten_number_by_form(&hand, Form::ThirteenOrphans);
    let best_other = calc_shanten_number_by_form(&hand, Form::Normal)
        .min(calc_shanten_number_by_form(&hand, Form::SevenPairs));

    // #158: 8〜9種は他形と同等以上に近ければ採用（高く評価）
    if kinds >= 8 && orphans <= best_other {
        return true;
    }

    // #159: 大きく負けているなら7種から、多少遠くても役満を狙う
    if is_far_behind(state) && orphans.as_i32() <= best_other.as_i32() + 1 {
        return true;
    }

    // #158: 7種は通常手に見込みがない（5向聴以上）ときのみ候補に留める
    orphans <= best_other && best_other.as_i32() >= 5
}

/// 大きく負けているか（#159: 役満狙いの価値が上がる点棒状況）
///
/// ラス目、またはトップとの点差が16000点以上ある場合。
pub(crate) fn is_far_behind(state: &CpuGameState) -> bool {
    let my_idx = CpuGameState::wind_to_index(state.my_seat_wind);
    let my_score = state.scores[my_idx];
    let top = *state.scores.iter().max().unwrap_or(&my_score);
    let is_last = state
        .scores
        .iter()
        .enumerate()
        .all(|(i, &s)| i == my_idx || s >= my_score);
    (top - my_score) >= 16000 || (is_last && top - my_score >= 8000)
}

/// 対子が「横に伸びにくい」か（#156）
///
/// 字牌・么九牌、または前後2つ以内に他の牌がない孤立した数牌対子は
/// 順子化しにくく、七対子向きの対子として扱う。
fn is_stiff_pair(counts: &[u8; 34], tile_type: TileType) -> bool {
    if tile_type >= 27 {
        return true; // 字牌
    }
    let pos = (tile_type % 9) as i32;
    if pos == 0 || pos == 8 {
        return true; // 么九牌
    }
    let suit_start = tile_type - tile_type % 9;
    let near = |offset: i32| -> bool {
        let q = pos + offset;
        (0..9).contains(&q) && q != pos && counts[(suit_start + q as TileType) as usize] > 0
    };
    !(near(-2) || near(-1) || near(1) || near(2))
}

/// #154/#155/#156: 選択した路線（一般形/七対子）に沿わない打牌を減点する
///
/// 総合向聴数（全形のmin）は対子が増えるだけで七対子に引っ張られるため、
/// 路線の形での向聴数と総合向聴数の差分をペナルティにして、
/// 実質的に「選択路線の向聴数」で打牌をランク付けする。
fn route_lock_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let route = preferred_form(ctx.state);

    // 候補を除いた残り手牌で、路線の形の向聴数を計算
    let mut remaining = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        remaining.push(drawn);
    }
    let Some(pos) = remaining.iter().position(|t| *t == c.tile) else {
        return 0.0;
    };
    remaining.remove(pos);

    let hand = Hand::new_with_melds(remaining, ctx.state.my_melds_for_analysis(), None);
    let target = calc_shanten_number_by_form(&hand, route);
    let overall = calc_shanten_number(&hand);

    let diff = (target.as_i32() - overall.as_i32()).max(0);
    -(diff as f64) * 100.0
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
///   （対々和・喰いタンの見込み条件は中以上で #157/#164 により厳しくなる）
/// - 安くて遠い仕掛けの抑制（#165, 中以上）: 打点要素がなく2向聴以上の
///   仕掛けはしない（親番は例外）
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

    if let Some((hand_after, melds_after)) = hand_after_pon(ctx.state, called_tile) {
        // 役なし鳴き禁止（弱以上）。
        // 対々和・喰いタンの見込みは中以上で #157/#164 の厳しい条件を使う
        if !has_yaku_prospect(
            &hand_after,
            &melds_after,
            ctx.state.my_seat_wind,
            ctx.state.prevailing_wind,
            ctx.config.level >= CpuLevel::Normal,
        ) {
            return CallJudgement::Forbid;
        }

        // 安くて遠い仕掛けは控える（#165, 中以上）
        if ctx.config.level >= CpuLevel::Normal
            && is_cheap_distant_call(
                ctx.state,
                &hand_after,
                &melds_after,
                ctx.config.level >= CpuLevel::Strong,
            )
        {
            return CallJudgement::Forbid;
        }
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
///   （対々和・喰いタンの見込み条件は中以上で #157/#164 により厳しくなる）
/// - 安くて遠い仕掛けの抑制（#165, 中以上）
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

    if let Some((hand_after, melds_after)) = hand_after_chi(ctx.state, called_tile, hand_tiles) {
        // 役なし鳴き禁止（弱以上）。
        // 対々和・喰いタンの見込みは中以上で #157/#164 の厳しい条件を使う
        if !has_yaku_prospect(
            &hand_after,
            &melds_after,
            ctx.state.my_seat_wind,
            ctx.state.prevailing_wind,
            ctx.config.level >= CpuLevel::Normal,
        ) {
            return CallJudgement::Forbid;
        }

        // 安くて遠い仕掛けは控える（#165, 中以上）
        if ctx.config.level >= CpuLevel::Normal
            && is_cheap_distant_call(
                ctx.state,
                &hand_after,
                &melds_after,
                ctx.config.level >= CpuLevel::Strong,
            )
        {
            return CallJudgement::Forbid;
        }
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

// ============================================================================
// 押し引きの定石（#178）
// ============================================================================

/// 押し引きに対する定石の判定結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushJudgement {
    /// 押すべき（攻撃続行）
    Push,
    /// 降りるべき
    Fold,
    /// 定石では決まらない（従来の判断に委ねる）
    Neutral,
}

/// 脅威がいる局面で押すか降りるかの定石判定（#178, 中以上）
///
/// - 聴牌: 良形で「高打点・親・脅威1人」のいずれかなら押す。
///   愚形かつ安手なら降りる（従来は聴牌なら無条件に押していた）。
///   親は連荘価値があるため愚形安手でも単独脅威には判断を保留する（#190）。
///   供託・本場が多い局では安手聴牌の価値が上がる（#191, 強以上）。
/// - 後半のトップ目は満貫級の良形聴牌以外は降りる（#188）。
/// - 2向聴: 高打点で脅威が1人なら押し続ける。大きく負けている場合は
///   打点の基準を下げて押す（#189: ラス目は打点寄り）。
/// - それ以外は従来の判断（性格・撤退閾値）に委ねる。
pub fn judge_push(ctx: &CallContext, threat_count: usize) -> PushJudgement {
    if !ctx.config.heuristics_enabled || ctx.config.level < CpuLevel::Normal || threat_count == 0 {
        return PushJudgement::Neutral;
    }

    let mut all_tiles = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        all_tiles.push(drawn);
    }
    let melds = ctx.state.my_melds_for_analysis();
    let hand = Hand::new_with_melds(all_tiles.clone(), melds.clone(), None);
    let shanten = calc_shanten_number(&hand);

    if shanten.is_ready_or_won() {
        // 聴牌: 最も広い聴牌を取る打牌を選んだときの待ちの形と打点で判断する
        let visible = ctx.state.visible_tile_counts();
        let mut best_waits = 0u32;
        let mut best_han = 0u32;
        for i in 0..all_tiles.len() {
            let mut remaining = all_tiles.clone();
            remaining.remove(i);
            let h = Hand::new_with_melds(remaining.clone(), melds.clone(), None);
            if !calc_shanten_number(&h).is_ready() {
                continue;
            }
            let waits = waiting_tiles(&remaining, &melds);
            let count: u32 = waits
                .iter()
                .map(|&t| 4u32.saturating_sub(visible[t as usize] as u32))
                .sum();
            let han = waits
                .iter()
                .filter_map(|&w| estimate_ron_han(ctx.state, &remaining, &melds, w))
                .max()
                .unwrap_or(0);
            if count > best_waits {
                best_waits = count;
                best_han = han;
            }
        }

        let good_shape = best_waits >= 6;
        // 門前ならリーチ・裏ドラなどの上積みを見込む
        let value_han = best_han + u32::from(ctx.state.my_melds().is_empty());
        let high_value = value_han >= 4;
        let dealer = ctx.state.my_seat_wind == Wind::East;

        // #188: 後半のトップ目は放銃回避を最優先する。
        // 満貫級の良形聴牌だけは押す
        if ctx.state.is_top() && ctx.state.is_second_half() {
            return if good_shape && high_value {
                PushJudgement::Push
            } else {
                PushJudgement::Fold
            };
        }

        if good_shape && (high_value || dealer || threat_count == 1) {
            return PushJudgement::Push;
        }
        if !good_shape && !high_value {
            // #190: 親は連荘価値があるため、単独脅威には判断を保留する
            // （従来の判断 = 聴牌なら押す、に委ねる）
            if dealer && threat_count == 1 {
                return PushJudgement::Neutral;
            }
            // #191（強以上）: 供託・本場が大きければ安手聴牌でも降り推奨はしない
            let stakes = ctx.state.riichi_sticks as i32 * 1000 + ctx.state.honba as i32 * 300;
            if ctx.config.level >= CpuLevel::Strong && stakes >= 2000 {
                return PushJudgement::Neutral;
            }
            return PushJudgement::Fold;
        }
        return PushJudgement::Neutral;
    }

    // #188: 後半のトップ目は聴牌以外では押さない
    if ctx.state.is_top() && ctx.state.is_second_half() {
        return PushJudgement::Fold;
    }

    // 2向聴: 高打点（推定値が満貫級）で脅威が1人なら押し続ける。
    // 大きく負けている場合は基準を下げる（#189: 高打点ルートを取りに行く）
    let value_threshold = if is_far_behind(ctx.state) { 4.0 } else { 6.0 };
    if shanten.as_i32() == 2
        && threat_count == 1
        && estimate_hand_value(&all_tiles, ctx.state) >= value_threshold
    {
        return PushJudgement::Push;
    }

    PushJudgement::Neutral
}

// ============================================================================
// リーチ・ダマ判断の定石（#168〜#172）
// ============================================================================

/// リーチ宣言に対する定石の判定結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiichiJudgement {
    /// リーチすべき
    Declare,
    /// ダマ（宣言しない）にすべき
    Damaten,
    /// 定石では決まらない（従来の積極度判断に委ねる）
    Neutral,
}

/// リーチすべきかの定石判定
///
/// `riichi_discard` はリーチ宣言牌（`None` はツモ切り）。
///
/// 適用される定石:
/// - #168（弱以上）: 役なし門前聴牌は基本的にリーチする
///   （リーチしないと和了できないため）
/// - #170（中以上）: 全ての待ちでダマでも満貫以上あるならダマにする
/// - #169（弱以上）: 先制の良形聴牌（残り待ち6枚以上）はリーチする
/// - #172（強以上）: 序盤の愚形聴牌で良形変化が多ければ一巡待つ
/// - #171（中以上）: 愚形の安手リーチは早巡の先制なら打ち、中終盤は控える
pub fn judge_riichi(ctx: &CallContext, riichi_discard: Option<Tile>) -> RiichiJudgement {
    if !ctx.config.heuristics_enabled {
        return RiichiJudgement::Neutral;
    }

    // リーチ後の13枚を構築
    let mut remaining = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        remaining.push(drawn);
    }
    let Some(target) = riichi_discard.or(ctx.state.my_drawn) else {
        return RiichiJudgement::Neutral;
    };
    let Some(pos) = remaining.iter().position(|t| *t == target) else {
        return RiichiJudgement::Neutral;
    };
    remaining.remove(pos);

    let melds = ctx.state.my_melds_for_analysis();
    let waits = waiting_tiles(&remaining, &melds);
    if waits.is_empty() {
        return RiichiJudgement::Neutral; // 聴牌していない（想定外）
    }

    let visible = ctx.state.visible_tile_counts();
    let wait_count: u32 = waits
        .iter()
        .map(|&t| 4u32.saturating_sub(visible[t as usize] as u32))
        .sum();

    // 各待ちでの「リーチなし・ロン和了」の翻数（役なしなら None）
    let values: Vec<Option<u32>> = waits
        .iter()
        .map(|&w| estimate_ron_han(ctx.state, &remaining, &melds, w))
        .collect();

    // #168（弱以上）: どの待ちでも役がない → リーチしないと和了できない
    if values.iter().all(Option::is_none) {
        return RiichiJudgement::Declare;
    }

    // #170（中以上）: 全ての待ちでダマでも満貫以上 → リーチ棒・放銃リスクを
    // 取らず出和了しやすさを優先する。
    // ただし大きく負けている場合は打点を伸ばす方が価値が高いので
    // リーチに回す（#189: ラス目は打点寄り）
    if ctx.config.level >= CpuLevel::Normal
        && !is_far_behind(ctx.state)
        && values.iter().all(|v| matches!(v, Some(han) if *han >= 5))
    {
        return RiichiJudgement::Damaten;
    }

    let my_idx = CpuGameState::wind_to_index(ctx.state.my_seat_wind);
    let opponent_riichi = ctx
        .state
        .player_riichi
        .iter()
        .enumerate()
        .any(|(i, &r)| i != my_idx && r);

    // #169（弱以上）: 先制の良形聴牌はリーチで打点を作る
    if wait_count >= 6 && !opponent_riichi {
        return RiichiJudgement::Declare;
    }

    // 愚形（残り待ち4枚以下）の判断
    if wait_count <= 4 {
        let turn = ctx.state.turn();

        // #172（強以上）: 序盤の愚形は良形変化が多ければ一巡待つ
        if ctx.config.level >= CpuLevel::Strong
            && turn <= 6
            && good_shape_upgrade_draws(&remaining, &visible) >= 12
        {
            return RiichiJudgement::Damaten;
        }

        if ctx.config.level >= CpuLevel::Normal {
            let max_han = values.iter().flatten().max().copied().unwrap_or(0);
            // 安手（ダマ2翻以下）の愚形は中終盤では控える
            if max_han <= 2 && turn >= 10 {
                return RiichiJudgement::Damaten;
            }
            // 早巡の先制なら愚形でもリーチ（#171）
            if !opponent_riichi && turn <= 8 {
                return RiichiJudgement::Declare;
            }
        }
    }

    RiichiJudgement::Neutral
}

/// 13枚の手牌の待ち牌の残り枚数合計を数える
///
/// リーチ宣言牌の選択（待ちの広い聴牌を選ぶ）に使用する。
pub(crate) fn remaining_wait_count(remaining: &[Tile], melds: &[Meld], visible: &[u8; 34]) -> u32 {
    waiting_tiles(remaining, melds)
        .iter()
        .map(|&t| 4u32.saturating_sub(visible[t as usize] as u32))
        .sum()
}

/// 13枚の手牌（副露込み）の待ち牌を列挙する
fn waiting_tiles(remaining: &[Tile], melds: &[Meld]) -> Vec<TileType> {
    (0..Tile::LEN as TileType)
        .filter(|&t| {
            let hand = Hand::new_with_melds(remaining.to_vec(), melds.to_vec(), Some(Tile::new(t)));
            calc_shanten_number(&hand).has_won()
        })
        .collect()
}

/// 「リーチなし・ロン和了」を仮定した翻数（ドラ込み）を計算する
///
/// 役がない（ロン和了できない）場合は `None`。
/// 裏ドラ・一発は不確定なので含めない。
fn estimate_ron_han(
    state: &CpuGameState,
    remaining: &[Tile],
    melds: &[Meld],
    wait: TileType,
) -> Option<u32> {
    let win_tile = Tile::new(wait);
    let hand = Hand::new_with_melds(remaining.to_vec(), melds.to_vec(), Some(win_tile));
    let analyzer = HandAnalyzer::new(&hand).ok()?;
    if !analyzer.shanten.has_won() {
        return None;
    }

    let mut status = Status::new();
    status.is_self_picked = false;
    status.player_wind = state.my_seat_wind;
    status.prevailing_wind = state.prevailing_wind;
    status.has_claimed_open = melds.iter().any(|m| m.from != MeldFrom::Myself);
    status.is_dealer = state.my_seat_wind == Wind::East;
    status.kan_count = melds
        .iter()
        .filter(|m| matches!(m.category, MeldType::Kan | MeldType::Kakan))
        .count() as u32;

    let result = calculate_score(&analyzer, &hand, &status, &Settings::new())
        .ok()
        .flatten()?;

    // ドラ・赤ドラを加算（裏ドラは不明なので含めない）
    let mut dora = 0u32;
    for t in remaining
        .iter()
        .chain(melds.iter().flat_map(|m| m.tiles.iter()))
        .chain(std::iter::once(&win_tile))
    {
        if t.is_red_dora() {
            dora += 1;
        }
        for indicator in &state.dora_indicators {
            if dora_indicator_to_dora(indicator.get()) == t.get() {
                dora += 1;
            }
        }
    }

    Some(result.han + dora)
}

/// 良形変化につながるツモの残り枚数を概算する（#172用の近似）
///
/// 手牌の数牌に隣接して新たな両面ターツを作る牌の残り枚数を数える。
/// 完成面子の隣も数えるため過大評価気味だが、「変化の多い手」の
/// 判定には十分な精度とする。
fn good_shape_upgrade_draws(remaining: &[Tile], visible: &[u8; 34]) -> u32 {
    let mut counts = [0u8; 34];
    for t in remaining {
        counts[t.get() as usize] += 1;
    }

    let mut counted = [false; 34];
    let mut total = 0u32;
    for tile_type in 0..27usize {
        if counts[tile_type] == 0 {
            continue;
        }
        let pos = (tile_type % 9) as i32;
        let suit_start = tile_type - tile_type % 9;
        for offset in [-1i32, 1] {
            let q = pos + offset;
            if !(0..9).contains(&q) {
                continue;
            }
            let neighbor = suit_start + q as usize;
            if counts[neighbor] > 0 || counted[neighbor] {
                continue;
            }
            // 新たにできるターツが両面か（下端が2〜7の位置）
            let pair_low = pos.min(q);
            if !(1..=6).contains(&pair_low) {
                continue;
            }
            counted[neighbor] = true;
            total += 4u32.saturating_sub(visible[neighbor] as u32);
        }
    }
    total
}

/// #165: 安くて遠い仕掛けか（中以上）
///
/// 鳴いた後も2向聴以上で、打点要素（ドラ・赤ドラ・役牌・染め手）が
/// 何もない仕掛けは、守備力低下のデメリットの方が大きいため控える。
/// 親は連荘価値があるため例外とする。
///
/// 点棒状況による例外・強化（#187/#191）:
/// - オーラスで安手の和了でも順位が上がるなら速度優先（例外）
/// - 供託・本場が大きい局は安手和了の価値が上がる（強以上、例外）
/// - オーラスで満貫級が必要なら、近い仕掛けでも安手なら控える（強化）
fn is_cheap_distant_call(
    state: &CpuGameState,
    hand_after: &[Tile],
    melds_after: &[Meld],
    consider_stakes: bool,
) -> bool {
    // 親番は例外（連荘価値がある）
    if state.my_seat_wind == Wind::East {
        return false;
    }

    // #187: オーラスで安手の和了でも順位が上がるなら速度を優先する
    if state.is_final_round() && state.gap_to_next_rank().is_some_and(|gap| gap <= 3900) {
        return false;
    }

    // #191（強以上）: 供託・本場が大きければ安手和了にも価値がある
    let stakes = state.riichi_sticks as i32 * 1000 + state.honba as i32 * 300;
    if consider_stakes && stakes >= 2000 {
        return false;
    }

    // #187: オーラスで満貫級が必要な点差なら、安手は近い仕掛けでも控える
    let needs_big_win =
        state.is_final_round() && state.gap_to_next_rank().is_some_and(|gap| gap >= 8000);

    // 鳴いた後も2向聴以上か（遠い仕掛けか）
    let hand = Hand::new_with_melds(hand_after.to_vec(), melds_after.to_vec(), None);
    if !needs_big_win && calc_shanten_number(&hand).as_i32() < 2 {
        return false;
    }

    // 打点要素: ドラ・赤ドラ
    let all_tiles = hand_after
        .iter()
        .chain(melds_after.iter().flat_map(|m| m.tiles.iter()));
    let mut counts = [0u8; 34];
    for t in all_tiles {
        if t.is_red_dora() {
            return false;
        }
        for indicator in &state.dora_indicators {
            if dora_indicator_to_dora(indicator.get()) == t.get() {
                return false;
            }
        }
        counts[t.get() as usize] += 1;
    }

    // 打点要素: 役牌対子以上
    for yh in get_yakuhai_types(state.my_seat_wind, state.prevailing_wind) {
        if counts[yh as usize] >= 2 {
            return false;
        }
    }

    // 打点要素: 染め手（数牌が1色に収まっている）
    let mut suits_used = [false; 3];
    for (tile_type, &count) in counts.iter().enumerate().take(27) {
        if count > 0 {
            suits_used[tile_type / 9] = true;
        }
    }
    if suits_used.iter().filter(|&&u| u).count() <= 1 {
        return false;
    }

    true
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
/// - 断么九: 副露が全て中張牌で、手牌の么九牌が少ない
/// - 混一色/清一色: 数牌が1色に収まっている
/// - 対々和: 副露が全て刻子系で、刻子系ブロックが十分にある
///
/// `strict` が true（中以上）の場合、より厳しい条件を使う:
/// - 対々和（#157）: 副露 + 手牌の対子・刻子が4ブロック以上
/// - 断么九（#164）: 手牌の么九牌が2枚以下、かつタンヤオ圏内に
///   複数ブロックが収まっている
///
/// false（弱）の場合は従来どおりの緩い条件で判定する。
///
/// チャンタ系などの稀な役は考慮しない（見込みなしと誤判定しても
/// 「鳴かない」側に倒れるだけで安全）。
pub fn has_yaku_prospect(
    hand_tiles: &[Tile],
    melds: &[Meld],
    seat_wind: Wind,
    prevailing_wind: Wind,
    strict: bool,
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
        if strict {
            // #164（中以上）: 么九牌が多い手から無理に喰いタンへ向かわない。
            // 么九牌2枚以下、かつタンヤオ圏内の牌で複数ブロックが
            // 構成できる場合のみ見込みとする。
            if terminal_honor_count <= 2 {
                let mut simple_counts = [0u8; 34];
                for t in hand_tiles {
                    if !t.is_1_9_honor() {
                        simple_counts[t.get() as usize] += 1;
                    }
                }
                if greedy_block_count(simple_counts) >= 2 {
                    return true;
                }
            }
        } else if terminal_honor_count <= 3 {
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
        let mut hand_counts = [0u8; 34];
        for t in hand_tiles {
            hand_counts[t.get() as usize] += 1;
        }
        if strict {
            // #157（中以上）: 副露 + 手牌の対子・刻子で4ブロック以上あるときだけ
            // 対々和を候補にする（2〜3トイツから無理に向かわない）
            let pair_or_triplet_types = hand_counts.iter().filter(|&&c| c >= 2).count();
            if melds.len() + pair_or_triplet_types >= 4 {
                return true;
            }
        } else {
            let hand_singles = hand_counts.iter().filter(|&&c| c == 1).count();
            if hand_singles <= 2 {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
#[path = "heuristics_tests.rs"]
mod tests;
