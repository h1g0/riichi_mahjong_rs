//! Heuristics framework (issue #142).
//!
//! Human-style discard wisdom is expressed as score adjustments on
//! discard candidates, with only the heuristics enabled for the CPU's
//! level applied. Each heuristic is a `DiscardHeuristic` registered in
//! `DISCARD_HEURISTICS` rather than a hard-coded branch, which makes
//! per-level toggling and per-heuristic testing possible.

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{
    HandAnalyzer, calc_shanten_number, calc_shanten_number_by_form,
};
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::hand_info::status::Status;
use mahjong_core::scoring::score::calculate_score;
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora_in};
use mahjong_core::winning_hand::name::Form;

use super::client::{CpuConfig, CpuLevel, is_yakuhai};
use super::defense::ORPHAN_TYPES;
use super::evaluator::{DiscardCandidate, estimate_hand_value, get_yakuhai_types};
use super::state::CpuGameState;

/// Context for computing discard adjustments.
pub struct DiscardContext<'a> {
    /// The game state as the CPU sees it
    pub state: &'a CpuGameState,
    /// CPU configuration
    pub config: &'a CpuConfig,
    /// Whether we are pushing; false means defense takes priority
    pub attacking: bool,
}

/// One heuristic.
///
/// `apply` returns an adjustment to how good discarding the candidate is:
/// positive makes the tile easier to discard, negative harder. The scale
/// matches `select_best_discard`'s base score (one shanten step = 100.0).
pub struct DiscardHeuristic {
    /// Name, for logs and tests
    pub name: &'static str,
    /// Minimum level at which the heuristic activates
    pub min_level: CpuLevel,
    /// The adjustment function
    pub apply: fn(&DiscardContext, &DiscardCandidate) -> f64,
}

/// Registry of discard heuristics (issue #142).
///
/// Adjustments are summed, so registration order does not matter.
pub const DISCARD_HEURISTICS: &[DiscardHeuristic] = &[
    // #147: discard isolated honours and terminals first (weak+).
    DiscardHeuristic {
        name: "isolated-honour-terminal-first",
        min_level: CpuLevel::Weak,
        apply: isolated_tile_bonus,
    },
    // #148: prefer two-sided shapes over edge/closed shapes (weak+).
    DiscardHeuristic {
        name: "protect-ryanmen-shapes",
        min_level: CpuLevel::Weak,
        apply: shape_protection_bonus,
    },
    // #152: do not discard dora carelessly (weak+).
    DiscardHeuristic {
        name: "protect-dora",
        min_level: CpuLevel::Weak,
        apply: dora_protection_bonus,
    },
    // #173/#174/#176: when defending, genbutsu comes first (weak+).
    DiscardHeuristic {
        name: "genbutsu-first-when-defending",
        min_level: CpuLevel::Weak,
        apply: defense_safety_bonus,
    },
    // #149: aim for five blocks in a standard hand (normal+).
    DiscardHeuristic {
        name: "five-block-surplus",
        min_level: CpuLevel::Normal,
        apply: five_block_bonus,
    },
    // #151: do not casually break the only pair candidate (normal+).
    DiscardHeuristic {
        name: "protect-sole-pair",
        min_level: CpuLevel::Normal,
        apply: sole_pair_protection,
    },
    // #153: adjust by how many useful tiles remain visible (normal+).
    DiscardHeuristic {
        name: "dismantle-dead-shapes",
        min_level: CpuLevel::Normal,
        apply: dead_shape_bonus,
    },
    // #150: with three or more pairs, break some up (strong).
    DiscardHeuristic {
        name: "break-excess-pairs",
        min_level: CpuLevel::Strong,
        apply: excess_pair_bonus,
    },
    // #154/#155/#156: discard along the chosen route,
    // seven pairs vs standard (normal+).
    DiscardHeuristic {
        name: "follow-hand-route",
        min_level: CpuLevel::Normal,
        apply: route_lock_bonus,
    },
    // #186: never discard dangerously on the final discard (normal+).
    DiscardHeuristic {
        name: "safe-last-discard",
        min_level: CpuLevel::Normal,
        apply: last_discard_safety_bonus,
    },
];

// ============================================================================
// Discard heuristic implementations.
// ============================================================================

/// Tile-kind counts of the hand after removing the candidate,
/// i.e. the shape that would remain after the discard.
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

/// #147: make isolated honours and terminals easier to discard.
///
/// In a scattered early hand the discard preference runs guest wind >
/// terminal > value honour > 2/8. Value honours outrank terminals as
/// keeps because pairing one yields a yaku. Isolated inside tiles get no
/// bonus and are therefore kept.
fn isolated_tile_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    let counts = remaining_counts(ctx.state, c.tile);
    let tt = c.tile.get();

    if counts[tt as usize] >= 1 {
        return 0.0;
    }

    if tt >= 27 {
        if is_yakuhai(tt, ctx.state.my_seat_wind, ctx.state.round_wind) {
            8.0
        } else {
            16.0
        }
    } else {
        // A suit tile with a neighbour within two ranks is a partial
        // sequence candidate, not isolated.
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
            _ => 0.0,      // Inside tiles are never discarded carelessly.
        }
    }
}

/// #148: protect two-sided shapes; loosen edge and closed shapes.
fn shape_protection_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let tt = c.tile.get();
    if tt >= 27 {
        return 0.0; // Honours form no sequences.
    }

    let counts = remaining_counts(ctx.state, c.tile);
    if counts[tt as usize] >= 1 {
        return 0.0; // Pairs/triplets are judged elsewhere.
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
        return 0.0; // Middle of a sequence; the shanten term covers it.
    }

    if lower || upper {
        // Two adjacent tiles: the lower end position tells two-sided
        // (0-indexed start 1..=6) from edge shapes.
        let pair_low = if lower { pos - 1 } else { pos };
        let two_sided = (1..=6).contains(&pair_low);
        return if two_sided {
            -6.0 // Keep two-sided shapes.
        } else {
            3.0 // Edge shapes (12/89) may go.
        };
    }

    if has(-2) || has(2) {
        return 3.0; // Closed shapes are weak too.
    }

    0.0
}

/// #152: do not discard dora or red fives carelessly.
///
/// While pushing, each dora carries a penalty that keeps it in hand.
/// No adjustment while defending, where safety must win.
fn dora_protection_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let mut dora_count = u32::from(c.tile.is_red_dora());
    for indicator in &ctx.state.dora_indicators {
        if dora_indicator_to_dora_in(indicator.get(), ctx.state.three_player) == c.tile.get() {
            dora_count += 1;
        }
    }
    -(dora_count as f64) * 12.0
}

/// #173/#174/#176: when defending, safety dominates (full fold).
///
/// A heavy weight on safety orders discards genbutsu > suji/honours >
/// off-suji terminals > off-suji inside tiles (456 most dangerous).
/// Weight 300 equals three shanten steps, so genbutsu is discarded even
/// at the cost of tenpai.
///
/// #179 (strong): when folding at tenpai/1-shanten the weight drops to
/// 150. A suji-sized safety difference (0.25 -> 37.5 points) can then no
/// longer beat one shanten step (100), so instead of wrecking the shape
/// with genbutsu the CPU plays safe-ish tiles (suji, honours) and keeps a
/// path back to tenpai - "mawashi" play. The gap to an off-suji inside
/// tile (0.85 -> 127.5) still exceeds a shanten step, so the shape is
/// never protected by pushing a dangerous tile.
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
        // #179 (strong): mawashi play.
        if ctx.config.level >= CpuLevel::Strong {
            weight = 150.0;
        }
        // #184 (normal+): near the exhaustive draw, chase a formal tenpai
        // while still discarding safe-ish tiles.
        if ctx.config.level >= CpuLevel::Normal && ctx.state.remaining_tiles <= 8 {
            weight = 150.0;
            // #185 (strong): the dealer, or a non-leader in the final
            // hand, values keeping tenpai even more.
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

/// #186: never discard dangerously on the hand's final discard.
///
/// With the wall empty this discard is the last action of the hand, so
/// advancing the hand is worthless; safety gets a heavy weight even while
/// pushing. Keeping formal tenpai (~100 points = one shanten step) still
/// wins over suji-sized safety differences, but never over pushing an
/// off-suji dangerous tile.
fn last_discard_safety_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if ctx.state.remaining_tiles > 0 {
        return 0.0;
    }

    let my_idx = CpuGameState::wind_to_index(ctx.state.my_seat_wind);
    let any_threat = (0..4).any(|i| {
        i != my_idx && (ctx.state.player_riichi[i] || ctx.state.player_melds[i].len() >= 3)
    });
    if !any_threat {
        return 0.0;
    }

    c.safety * 200.0
}

/// Counts blocks in the whole hand (drawn tile and melds included).
///
/// Block = group (melds included) + pair + partial sequence. A winning
/// hand is 4 groups + 1 pair = 5 blocks, so 6+ is a surplus.
///
/// `HandAnalyzer` only records the five blocks the shanten needs (extras
/// become isolated tiles), so this counts greedily from tile-kind counts:
/// triplets, then sequences, pairs, and partial sequences.
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

/// Greedy block count from tile-kind counts:
/// triplets, sequences, pairs, then partial sequences.
fn greedy_block_count(mut counts: [u8; 34]) -> usize {
    let mut blocks = 0;

    for c in counts.iter_mut() {
        if *c >= 3 {
            *c -= 3;
            blocks += 1;
        }
    }

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

    for c in counts.iter_mut() {
        if *c >= 2 {
            *c -= 2;
            blocks += 1;
        }
    }

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

/// Tile kinds held exactly twice (pairs); three or more copies count as
/// triplets (or kan material) and are excluded.
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

/// #149: with six or more blocks, shed the weak ones
/// (edge/closed shapes, surplus pairs).
fn five_block_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }
    if count_blocks(ctx.state) < 6 {
        return 0.0;
    }

    let tt = c.tile.get();
    let counts = remaining_counts(ctx.state, c.tile);

    // With two or more pairs, one may be shed.
    if counts[tt as usize] == 1 && pair_types(ctx.state).len() >= 2 {
        return 4.0;
    }

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
            // Edge shapes may be shed.
            let pair_low = if lower { pos - 1 } else { pos };
            if !(1..=6).contains(&pair_low) {
                return 6.0;
            }
        } else if !lower && !upper && (has(-2) || has(2)) {
            // So may closed shapes.
            return 6.0;
        }
    }

    0.0
}

/// #151: never break the only pair candidate.
fn sole_pair_protection(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let pairs = pair_types(ctx.state);
    if pairs.len() != 1 || pairs[0] != c.tile.get() {
        return 0.0;
    }

    // A triplet can also supply the pair.
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

/// #153: dismantle shapes whose winning tiles are nearly dead.
///
/// Closed and edge shapes wait on one kind, so one remaining copy or
/// fewer makes them dead; a two-sided shape is treated the same when both
/// waits total two copies or fewer.
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
        return 0.0; // Pairs/triplets are out of scope.
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
        return 0.0; // Middle of a sequence.
    }

    if lower || upper {
        // Adjacent shape: two-sided waits on both ends, edge on one.
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
        // Closed shape: only the middle tile completes it.
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

/// #150 (strong): with three or more pairs, break up the ones that
/// convert to sequences most easily.
///
/// Applies only when the standard form is clearly closer than seven
/// pairs. Inside-tile pairs go first: the remaining tile becomes a
/// two-sided candidate. Honour pairs stay as pon material or the head.
fn excess_pair_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let pairs = pair_types(ctx.state);
    if pairs.len() < 3 || !pairs.contains(&c.tile.get()) {
        return 0.0;
    }

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
        return 0.0; // Keep honour pairs.
    }
    match tt % 9 {
        0 | 8 => 3.0,
        _ => 5.0, // The freed inside tile keeps sequence potential.
    }
}

/// Sums every active heuristic's adjustment for one candidate.
pub fn discard_adjustment(ctx: &DiscardContext, candidate: &DiscardCandidate) -> f64 {
    discard_adjustment_with(DISCARD_HEURISTICS, ctx, candidate)
}

/// Sums adjustments from an explicit registry.
///
/// Heuristics above the CPU's level are skipped; `heuristics_enabled`
/// false disables everything (for A/B comparison).
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

/// Picks the hand's main route: seven pairs, thirteen orphans, or the
/// standard form (#154/#155/#156, #158-#161).
///
/// - Any meld forces the standard form (the others are closed-only).
/// - The orphan-kind count and score situation may select the Thirteen
///   Orphans route (#158-#161).
/// - Fewer than four pairs means standard (#154).
/// - Even with four pairs, standard wins when it is closer or the pairs
///   sit in composite shapes (#155).
/// - When most pairs are "stiff" (honours, terminals, isolated suit
///   pairs that cannot extend sideways), head for seven pairs (#156).
pub(crate) fn preferred_form(state: &CpuGameState) -> Form {
    if !state.my_melds().is_empty() {
        return Form::Normal;
    }

    let mut all_tiles = state.my_hand.clone();
    if let Some(drawn) = state.my_drawn {
        all_tiles.push(drawn);
    }

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

    // Pair quality: mostly stiff pairs leans seven pairs.
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

/// Whether Thirteen Orphans should be the main route (#158-#161).
///
/// - #160: 10+ orphan kinds make it the main route.
/// - #158: 8-9 kinds qualify when at least as close as the other forms;
///   7 kinds only when the normal hand is hopeless (5+ shanten).
/// - #159: when far behind, chase from 7 kinds even if slightly farther —
///   a yakuman is worth the comeback.
/// - #161: a missing requirement that is dead (4 visible) makes the form
///   impossible; from mid-game, two or more missing kinds with at most
///   one copy left also abandon the chase. This is our own decision, so
///   all visibility including our own hand counts.
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

    // #161: dead-tile checks on missing requirements.
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

    if kinds >= 10 {
        return true;
    }

    let hand = Hand::new(all_tiles.to_vec(), None);
    let orphans = calc_shanten_number_by_form(&hand, Form::ThirteenOrphans);
    let best_other = calc_shanten_number_by_form(&hand, Form::Normal)
        .min(calc_shanten_number_by_form(&hand, Form::SevenPairs));

    if kinds >= 8 && orphans <= best_other {
        return true;
    }

    if is_far_behind(state) && orphans.as_i32() <= best_other.as_i32() + 1 {
        return true;
    }

    orphans <= best_other && best_other.as_i32() >= 5
}

/// Whether we are far behind (#159): last place, or 16000+ points off
/// the lead — situations where chasing a yakuman gains value.
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

/// Whether a pair is "stiff" (#156): honours, terminals, or isolated
/// suit pairs with no neighbour within two ranks rarely become
/// sequences, which suits seven pairs.
fn is_stiff_pair(counts: &[u8; 34], tile_type: TileType) -> bool {
    if tile_type >= 27 {
        return true; // honour
    }
    let pos = (tile_type % 9) as i32;
    if pos == 0 || pos == 8 {
        return true; // terminal
    }
    let suit_start = tile_type - tile_type % 9;
    let near = |offset: i32| -> bool {
        let q = pos + offset;
        (0..9).contains(&q) && q != pos && counts[(suit_start + q as TileType) as usize] > 0
    };
    !(near(-2) || near(-1) || near(1) || near(2))
}

/// #154/#155/#156: penalize discards that stray from the chosen route.
///
/// The overall shanten (min across forms) drifts towards seven pairs as
/// pairs accumulate, so the difference between the route's shanten and
/// the overall shanten becomes a penalty — effectively ranking discards
/// by the chosen route's shanten.
fn route_lock_bonus(ctx: &DiscardContext, c: &DiscardCandidate) -> f64 {
    if !ctx.attacking {
        return 0.0;
    }

    let route = preferred_form(ctx.state);

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
// Call heuristics.
//
// Unlike discard heuristics (summed adjustments), a call is a discrete
// decision, expressed as forbid / encourage / neutral. Forbid wins over
// encourage.
// ============================================================================

/// Context for call decisions.
pub struct CallContext<'a> {
    /// The game state as the CPU sees it
    pub state: &'a CpuGameState,
    /// CPU configuration
    pub config: &'a CpuConfig,
}

/// Heuristic verdict on a call or kan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallJudgement {
    /// The heuristics forbid the call
    Forbid,
    /// The heuristics encourage it, overriding personality/aggressiveness
    Encourage,
    /// The heuristics abstain; personality/aggressiveness decides
    Neutral,
}

/// Heuristic verdict on a pon.
///
/// Applies:
/// - avoid the bare pair (#166, weak+): never make a fourth meld;
/// - no yakuless calls (#162, weak+); the toitoi/kuitan prospects use
///   the stricter #157/#164 conditions from normal up;
/// - suppress cheap distant calls (#165, normal+): no yaku-less 2+
///   shanten calls (the dealer is exempt);
/// - pon value-honour pairs early (#163, weak+), whatever the
///   personality.
///
/// The caller has already verified the call lowers the shanten.
pub fn judge_pon(ctx: &CallContext, called_tile: Tile) -> CallJudgement {
    if !ctx.config.heuristics_enabled {
        return CallJudgement::Neutral;
    }

    if ctx.state.my_melds().len() >= 3 {
        return CallJudgement::Forbid;
    }

    if let Some((hand_after, melds_after)) = hand_after_pon(ctx.state, called_tile) {
        if !has_yaku_prospect(
            &hand_after,
            &melds_after,
            ctx.state.my_seat_wind,
            ctx.state.round_wind,
            ctx.config.level >= CpuLevel::Normal,
        ) {
            return CallJudgement::Forbid;
        }

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

    if is_yakuhai(
        called_tile.get(),
        ctx.state.my_seat_wind,
        ctx.state.round_wind,
    ) {
        return CallJudgement::Encourage;
    }

    CallJudgement::Neutral
}

/// Heuristic verdict on a chii.
///
/// Applies the same rules as `judge_pon` minus the value-honour
/// encouragement: avoid the bare pair (#166), no yakuless calls (#162,
/// with strict #157/#164 prospects from normal up), and suppress cheap
/// distant calls (#165, normal+).
///
/// The caller has already verified the call lowers the shanten.
pub fn judge_chi(ctx: &CallContext, called_tile: Tile, hand_tiles: [Tile; 2]) -> CallJudgement {
    if !ctx.config.heuristics_enabled {
        return CallJudgement::Neutral;
    }

    if ctx.state.my_melds().len() >= 3 {
        return CallJudgement::Forbid;
    }

    if let Some((hand_after, melds_after)) = hand_after_chi(ctx.state, called_tile, hand_tiles) {
        if !has_yaku_prospect(
            &hand_after,
            &melds_after,
            ctx.state.my_seat_wind,
            ctx.state.round_wind,
            ctx.config.level >= CpuLevel::Normal,
        ) {
            return CallJudgement::Forbid;
        }

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

/// Heuristic verdict on a concealed kan (#167, normal+).
///
/// - Never kan when it worsens the shanten (breaks the hand).
/// - Under an opponent's riichi, kan only when still tenpai afterwards:
///   the new dora indicator could inflate their hand.
pub fn judge_ankan(ctx: &CallContext, tile_type: TileType) -> CallJudgement {
    if !ctx.config.heuristics_enabled || ctx.config.level < CpuLevel::Normal {
        return CallJudgement::Neutral;
    }

    let mut all_tiles = ctx.state.my_hand.clone();
    if let Some(drawn) = ctx.state.my_drawn {
        all_tiles.push(drawn);
    }

    let melds = ctx.state.my_melds_for_analysis();

    let before_hand = Hand::new_with_melds(all_tiles.clone(), melds.clone(), None);
    let before = calc_shanten_number(&before_hand);

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

    if after > before {
        return CallJudgement::Forbid;
    }

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
// Push/fold heuristics (#178).
// ============================================================================

/// Heuristic verdict on pushing vs folding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushJudgement {
    /// Keep attacking
    Push,
    /// Fold
    Fold,
    /// Undecided; the legacy judgement applies
    Neutral,
}

/// Push-or-fold verdict when threats exist (#178, normal+).
///
/// - Tenpai: push a good shape with any of high value / dealer / a single
///   threat; fold a bad-shape cheap hand (tenpai used to push
///   unconditionally). The dealer defers to the legacy judgement against
///   a single threat because continuation has value (#190); big table
///   stakes raise the value of a cheap tenpai (#191, strong).
/// - A leader in the second half folds everything but a mangan-class
///   good-shape tenpai (#188).
/// - 2-shanten: keep pushing a high-value hand against a single threat;
///   when far behind the value bar drops (#189).
/// - Everything else defers to the personality/threshold judgement.
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
        // At tenpai, judge on the widest tenpai's wait shape and value.
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
        // A closed hand can add riichi and ura dora.
        let value_han = best_han + u32::from(ctx.state.my_melds().is_empty());
        let high_value = value_han >= 4;
        let dealer = ctx.state.my_seat_wind == Wind::East;

        // #188: a second-half leader avoids deal-ins above all,
        // pushing only a mangan-class good shape.
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
            // #190: the dealer defers against a single threat
            // (the legacy judgement pushes at tenpai).
            if dealer && threat_count == 1 {
                return PushJudgement::Neutral;
            }
            // #191 (strong): big stakes remove the fold recommendation.
            let stakes = ctx.state.riichi_sticks as i32 * 1000 + ctx.state.honba as i32 * 300;
            if ctx.config.level >= CpuLevel::Strong && stakes >= 2000 {
                return PushJudgement::Neutral;
            }
            return PushJudgement::Fold;
        }
        return PushJudgement::Neutral;
    }

    // #188: a second-half leader never pushes below tenpai.
    if ctx.state.is_top() && ctx.state.is_second_half() {
        return PushJudgement::Fold;
    }

    // 2-shanten: keep pushing a mangan-class hand against one threat;
    // when far behind the bar drops (#189).
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
// Riichi vs damaten heuristics (#168-#172).
// ============================================================================

/// Heuristic verdict on declaring riichi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiichiJudgement {
    /// Declare riichi
    Declare,
    /// Stay damaten (do not declare)
    Damaten,
    /// Undecided; the aggressiveness judgement applies
    Neutral,
}

/// Heuristic verdict on declaring riichi.
///
/// `riichi_discard` is the declaration discard (`None` = tsumogiri).
///
/// Applies:
/// - #168 (weak+): a yakuless closed tenpai declares — without riichi it
///   cannot win at all;
/// - #170 (normal+): when every wait is mangan-class even damaten,
///   stay damaten;
/// - #169 (weak+): an uncontested good-shape tenpai (6+ waits left)
///   declares;
/// - #172 (strong): an early bad-shape tenpai with many good-shape
///   upgrades waits a turn;
/// - #171 (normal+): a cheap bad-shape riichi goes in early uncontested
///   turns but not mid-to-late game.
pub fn judge_riichi(ctx: &CallContext, riichi_discard: Option<Tile>) -> RiichiJudgement {
    if !ctx.config.heuristics_enabled {
        return RiichiJudgement::Neutral;
    }

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
        return RiichiJudgement::Neutral; // Not tenpai (unexpected).
    }

    let visible = ctx.state.visible_tile_counts();
    let wait_count: u32 = waits
        .iter()
        .map(|&t| 4u32.saturating_sub(visible[t as usize] as u32))
        .sum();

    let values: Vec<Option<u32>> = waits
        .iter()
        .map(|&w| estimate_ron_han(ctx.state, &remaining, &melds, w))
        .collect();

    // #168: no wait carries a yaku, so riichi is the only path to a win.
    if values.iter().all(Option::is_none) {
        return RiichiJudgement::Declare;
    }

    // #170: mangan-class on every wait even damaten - skip the deposit
    // and deal-in risk and stay easy to win off discards. When far
    // behind, growing the hand matters more, so declare instead (#189).
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

    // #169: an uncontested good shape declares for value.
    if wait_count >= 6 && !opponent_riichi {
        return RiichiJudgement::Declare;
    }

    // Bad shapes (4 or fewer waits left).
    if wait_count <= 4 {
        let turn = ctx.state.turn();

        if ctx.config.level >= CpuLevel::Strong
            && turn <= 6
            && good_shape_upgrade_draws(&remaining, &visible) >= 12
        {
            return RiichiJudgement::Damaten;
        }

        if ctx.config.level >= CpuLevel::Normal {
            let max_han = values.iter().flatten().max().copied().unwrap_or(0);
            // A cheap bad shape holds back mid-to-late game...
            if max_han <= 2 && turn >= 10 {
                return RiichiJudgement::Damaten;
            }
            // ...but declares in early uncontested turns (#171).
            if !opponent_riichi && turn <= 8 {
                return RiichiJudgement::Declare;
            }
        }
    }

    RiichiJudgement::Neutral
}

/// Total remaining copies of a 13-tile hand's waits; used to pick the
/// widest tenpai when choosing the riichi declaration discard.
pub(crate) fn remaining_wait_count(remaining: &[Tile], melds: &[Meld], visible: &[u8; 34]) -> u32 {
    waiting_tiles(remaining, melds)
        .iter()
        .map(|&t| 4u32.saturating_sub(visible[t as usize] as u32))
        .sum()
}

/// Enumerates the waits of a 13-tile hand (melds included).
fn waiting_tiles(remaining: &[Tile], melds: &[Meld]) -> Vec<TileType> {
    (0..Tile::LEN as TileType)
        .filter(|&t| {
            let hand = Hand::new_with_melds(remaining.to_vec(), melds.to_vec(), Some(Tile::new(t)));
            calc_shanten_number(&hand).has_won()
        })
        .collect()
}

/// Estimated han (dora included) of a hypothetical no-riichi ron win;
/// `None` when the hand has no yaku and cannot ron. Ura dora and ippatsu
/// are uncertain and excluded.
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
    status.is_self_drawn = false;
    status.seat_wind = state.my_seat_wind;
    status.round_wind = state.round_wind;
    status.has_claimed_open = melds.iter().any(|m| m.from != MeldFrom::Myself);
    status.is_dealer = state.my_seat_wind == Wind::East;
    status.kan_count = melds
        .iter()
        .filter(|m| matches!(m.category, MeldType::Kan | MeldType::Kakan))
        .count() as u32;

    let result = calculate_score(&analyzer, &hand, &status, &Settings::new())
        .ok()
        .flatten()?;

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
            if dora_indicator_to_dora_in(indicator.get(), state.three_player) == t.get() {
                dora += 1;
            }
        }
    }

    Some(result.han + dora)
}

/// Rough count of draws that upgrade the hand into a good shape (#172).
///
/// Counts remaining tiles adjacent to our suit tiles that would form a
/// new two-sided shape. Neighbours of completed groups also count, so it
/// overestimates a little - good enough for spotting "flexible" hands.
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
            // Only count it when the new shape is two-sided.
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

/// #165 (normal+): whether the call is cheap and distant.
///
/// A call that still leaves 2+ shanten with no value element (dora, red
/// five, value honour, flush) loses more in defense than it gains, so it
/// is suppressed. The dealer is exempt: continuation has value.
///
/// Score-situation adjustments (#187/#191):
/// - final hand where even a cheap win climbs a place: speed wins
///   (exemption);
/// - big table stakes raise cheap wins' value (strong, exemption);
/// - final hand needing a mangan-class win: suppress cheap calls even
///   when close (tightening).
fn is_cheap_distant_call(
    state: &CpuGameState,
    hand_after: &[Tile],
    melds_after: &[Meld],
    consider_stakes: bool,
) -> bool {
    if state.my_seat_wind == Wind::East {
        return false;
    }

    if state.is_final_round() && state.gap_to_next_rank().is_some_and(|gap| gap <= 3900) {
        return false;
    }

    let stakes = state.riichi_sticks as i32 * 1000 + state.honba as i32 * 300;
    if consider_stakes && stakes >= 2000 {
        return false;
    }

    let needs_big_win =
        state.is_final_round() && state.gap_to_next_rank().is_some_and(|gap| gap >= 8000);

    let hand = Hand::new_with_melds(hand_after.to_vec(), melds_after.to_vec(), None);
    if !needs_big_win && calc_shanten_number(&hand).as_i32() < 2 {
        return false;
    }

    // Value elements: dora and red fives.
    let all_tiles = hand_after
        .iter()
        .chain(melds_after.iter().flat_map(|m| m.tiles.iter()));
    let mut counts = [0u8; 34];
    for t in all_tiles {
        if t.is_red_dora() {
            return false;
        }
        for indicator in &state.dora_indicators {
            if dora_indicator_to_dora_in(indicator.get(), state.three_player) == t.get() {
                return false;
            }
        }
        counts[t.get() as usize] += 1;
    }

    // Value element: a value-honour pair or better.
    for yh in get_yakuhai_types(state.my_seat_wind, state.round_wind) {
        if counts[yh as usize] >= 2 {
            return false;
        }
    }

    // Value element: a flush (all suit tiles in one suit).
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

/// Builds the hand and melds after a pon; `None` when the hand lacks
/// two matching tiles.
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

/// Builds the hand and melds after a chii; `None` when the hand lacks
/// the two tiles.
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

/// Rough check that an open hand still has a yaku prospect (#162).
///
/// Considers the representative open-hand yaku:
/// - value honours: two or more in hand + melds;
/// - All Inside: melds all inside tiles with few orphans in hand;
/// - Common/Perfect Flush: all suit tiles in one suit;
/// - All Triplets: melds all triplet-like with enough triplet blocks.
///
/// With `strict` (normal+) the toitoi and kuitan conditions tighten:
/// - All Triplets (#157): melds + hand pairs/triplets total 4+ blocks;
/// - All Inside (#164): at most two orphans in hand and multiple blocks
///   already inside the tanyao range.
///
/// Rare yaku (chanta family etc.) are ignored: a false "no prospect"
/// only errs on the side of not calling, which is safe.
pub fn has_yaku_prospect(
    hand_tiles: &[Tile],
    melds: &[Meld],
    seat_wind: Wind,
    round_wind: Wind,
    strict: bool,
) -> bool {
    let mut counts = [0u8; 34];
    for t in hand_tiles {
        counts[t.get() as usize] += 1;
    }
    for meld in melds {
        for t in &meld.tiles {
            counts[t.get() as usize] += 1;
        }
    }

    for yh in get_yakuhai_types(seat_wind, round_wind) {
        if counts[yh as usize] >= 2 {
            return true;
        }
    }

    let melds_all_simple = melds
        .iter()
        .all(|m| m.tiles.iter().all(|t| !t.is_1_9_honour()));
    if melds_all_simple {
        let terminal_honour_count = hand_tiles.iter().filter(|t| t.is_1_9_honour()).count();
        if strict {
            // #164: never force kuitan from an orphan-heavy hand.
            if terminal_honour_count <= 2 {
                let mut simple_counts = [0u8; 34];
                for t in hand_tiles {
                    if !t.is_1_9_honour() {
                        simple_counts[t.get() as usize] += 1;
                    }
                }
                if greedy_block_count(simple_counts) >= 2 {
                    return true;
                }
            }
        } else if terminal_honour_count <= 3 {
            return true;
        }
    }

    let mut suits_used = [false; 3];
    for (tile_type, &count) in counts.iter().enumerate().take(27) {
        if count > 0 {
            suits_used[tile_type / 9] = true;
        }
    }
    if suits_used.iter().filter(|&&u| u).count() <= 1 {
        return true;
    }

    let melds_all_triplets = melds
        .iter()
        .all(|m| matches!(m.category, MeldType::Pon | MeldType::Kan | MeldType::Kakan));
    if melds_all_triplets && !melds.is_empty() {
        let mut hand_counts = [0u8; 34];
        for t in hand_tiles {
            hand_counts[t.get() as usize] += 1;
        }
        if strict {
            // #157: only chase toitoi with 4+ triplet-family blocks.
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
