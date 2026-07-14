//! Server-side scoring: wrappers over mahjong-core's scoring that judge
//! wins from a player's hand and the hand state, then compute the point
//! transfers.

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{self, HandAnalyzer};
use mahjong_core::hand_info::status::Status;
use mahjong_core::scoring::score::{
    DoraLabel, ScoreItem, ScoreResult, calculate_base_points, calculate_score, determine_rank,
    round_up_to_100,
};
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora_in};

use crate::player::Player;

/// Result of a win check.
#[derive(Debug)]
pub struct WinCheckResult {
    /// Whether the hand wins
    pub is_win: bool,
    /// Score details; Some only when the hand wins
    pub score_result: Option<ScoreResult>,
}

/// Checks whether the player's hand wins, under default rules.
pub fn check_win(
    player: &Player,
    round_wind: Wind,
    is_tsumo: bool,
    is_last_tile: bool,
    is_after_a_quad: bool,
) -> WinCheckResult {
    let settings = Settings::new();
    check_win_with_settings(
        player,
        round_wind,
        is_tsumo,
        is_last_tile,
        is_after_a_quad,
        &settings,
    )
}

/// Checks whether the player's hand wins, under the given rules.
pub fn check_win_with_settings(
    player: &Player,
    round_wind: Wind,
    is_tsumo: bool,
    is_last_tile: bool,
    is_after_a_quad: bool,
    settings: &Settings,
) -> WinCheckResult {
    let hand = &player.hand;

    let analyzer = match HandAnalyzer::new(hand) {
        Ok(a) => a,
        Err(_) => {
            return WinCheckResult {
                is_win: false,
                score_result: None,
            };
        }
    };

    if !analyzer.shanten.has_won() {
        return WinCheckResult {
            is_win: false,
            score_result: None,
        };
    }

    let mut status = Status::new();
    status.is_self_drawn = is_tsumo;
    status.seat_wind = player.seat_wind;
    status.round_wind = round_wind;
    status.has_claimed_riichi = player.is_riichi;
    status.is_double_riichi = player.is_double_riichi;
    status.is_unbroken = player.is_ippatsu;
    status.has_claimed_open = !player.is_menzen();
    status.is_dealer = player.is_dealer();
    status.is_first_turn = player.is_first_turn;
    status.is_last_tile_draw = is_last_tile && is_tsumo;
    status.is_last_tile_claim = is_last_tile && !is_tsumo;
    status.is_after_a_quad = is_after_a_quad;
    status.kan_count = player.kan_count() as u32;

    match calculate_score(&analyzer, hand, &status, settings) {
        Ok(Some(result)) => WinCheckResult {
            is_win: true,
            score_result: Some(result),
        },
        _ => WinCheckResult {
            is_win: false,
            score_result: None,
        },
    }
}

/// Checks whether the player can ron on a discard, under default rules.
///
/// Adds the discard to the 13-tile hand and judges the win.
/// The caller is responsible for the furiten check.
pub fn check_ron(
    player: &Player,
    discarded_tile: Tile,
    round_wind: Wind,
    is_last_tile: bool,
) -> WinCheckResult {
    let settings = Settings::new();
    check_ron_with_flags_and_settings(
        player,
        discarded_tile,
        round_wind,
        is_last_tile,
        false,
        &settings,
    )
}

/// Checks whether the player can ron on a discard, under the given rules.
pub fn check_ron_with_settings(
    player: &Player,
    discarded_tile: Tile,
    round_wind: Wind,
    is_last_tile: bool,
    settings: &Settings,
) -> WinCheckResult {
    check_ron_with_flags_and_settings(
        player,
        discarded_tile,
        round_wind,
        is_last_tile,
        false,
        settings,
    )
}

/// Checks whether the player can ron, with explicit state flags
/// (e.g. robbing a quad).
pub fn check_ron_with_flags_and_settings(
    player: &Player,
    discarded_tile: Tile,
    round_wind: Wind,
    is_last_tile: bool,
    is_robbing_a_quad: bool,
    settings: &Settings,
) -> WinCheckResult {
    let mut hand = player.hand.clone();
    hand.set_drawn(Some(discarded_tile));

    let analyzer = match HandAnalyzer::new(&hand) {
        Ok(a) => a,
        Err(_) => {
            return WinCheckResult {
                is_win: false,
                score_result: None,
            };
        }
    };

    if !analyzer.shanten.has_won() {
        return WinCheckResult {
            is_win: false,
            score_result: None,
        };
    }

    let mut status = Status::new();
    status.is_self_drawn = false;
    status.seat_wind = player.seat_wind;
    status.round_wind = round_wind;
    status.has_claimed_riichi = player.is_riichi;
    status.is_double_riichi = player.is_double_riichi;
    status.is_unbroken = player.is_ippatsu;
    status.has_claimed_open = !player.is_menzen();
    status.is_dealer = player.is_dealer();
    status.is_first_turn = player.is_first_turn;
    status.is_last_tile_draw = false;
    status.is_last_tile_claim = is_last_tile && !is_robbing_a_quad;
    status.is_robbing_a_quad = is_robbing_a_quad;
    status.kan_count = player.kan_count() as u32;

    match calculate_score(&analyzer, &hand, &status, settings) {
        Ok(Some(result)) => WinCheckResult {
            is_win: true,
            score_result: Some(result),
        },
        _ => WinCheckResult {
            is_win: false,
            score_result: None,
        },
    }
}

/// Returns the tile kinds the player is waiting on, for the furiten check.
///
/// With the 13-tile hand (drawn = None), tries every tile kind as the
/// drawn tile and collects those that complete the hand.
pub fn get_waiting_tiles(player: &Player) -> Vec<TileType> {
    let mut waiting = Vec::new();
    for tile_type in 0..Tile::LEN as u32 {
        let mut hand = player.hand.clone();
        hand.set_drawn(Some(Tile::new(tile_type)));

        if hand_analyzer::calc_shanten_number(&hand).has_won() {
            waiting.push(tile_type);
        }
    }
    waiting
}

/// Builds the fixed mangan score for a player who qualifies for Nagashi
/// Mangan at an exhaustive draw.
///
/// Qualification depends on discard history and is intentionally checked by
/// the round. This function only carries that authoritative result through
/// the core scoring pipeline.
pub fn check_nagashi_mangan(
    player: &Player,
    round_wind: Wind,
    settings: &Settings,
) -> WinCheckResult {
    let analyzer = match HandAnalyzer::new(&player.hand) {
        Ok(analyzer) => analyzer,
        Err(_) => {
            return WinCheckResult {
                is_win: false,
                score_result: None,
            };
        }
    };

    let mut status = Status::new();
    status.is_self_drawn = true;
    status.seat_wind = player.seat_wind;
    status.round_wind = round_wind;
    status.has_claimed_open = !player.is_menzen();
    status.is_dealer = player.is_dealer();
    status.is_nagashi_mangan = true;

    match calculate_score(&analyzer, &player.hand, &status, settings) {
        Ok(Some(result)) => WinCheckResult {
            is_win: true,
            score_result: Some(result),
        },
        _ => WinCheckResult {
            is_win: false,
            score_result: None,
        },
    }
}

/// Computes the point transfers for a tsumo win.
///
/// Three-player games use tsumo loss: per-person payments are unchanged
/// and the absent player's share is simply not received.
///
/// Returns each player's score delta (positive = gain); deltas always
/// sum to zero.
pub fn calculate_tsumo_score_deltas(
    winner: usize,
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    dealer_idx: usize,
    honba: usize,
    player_count: usize,
) -> [i32; 4] {
    calculate_tsumo_deltas_from_payments(
        winner,
        score_result.dealer_tsumo_all as i32,
        score_result.non_dealer_tsumo_dealer as i32,
        score_result.non_dealer_tsumo_non_dealer as i32,
        winner_is_dealer,
        dealer_idx,
        honba,
        player_count,
    )
}

#[allow(clippy::too_many_arguments)]
fn calculate_tsumo_deltas_from_payments(
    winner: usize,
    dealer_tsumo_all: i32,
    non_dealer_tsumo_dealer: i32,
    non_dealer_tsumo_non_dealer: i32,
    winner_is_dealer: bool,
    dealer_idx: usize,
    honba: usize,
    player_count: usize,
) -> [i32; 4] {
    let mut deltas = [0i32; 4];
    let honba_bonus = honba as i32 * 100;

    if winner_is_dealer {
        // Dealer tsumo: every non-dealer pays the same amount.
        let each_pay = dealer_tsumo_all + honba_bonus;
        for (i, delta) in deltas.iter_mut().enumerate().take(player_count) {
            if i == winner {
                *delta = each_pay * (player_count as i32 - 1);
            } else {
                *delta = -each_pay;
            }
        }
    } else {
        // Non-dealer tsumo: the dealer pays the larger share.
        let dealer_pay = non_dealer_tsumo_dealer + honba_bonus;
        let non_dealer_pay = non_dealer_tsumo_non_dealer + honba_bonus;
        let mut total_gain = 0i32;
        for (i, delta) in deltas.iter_mut().enumerate().take(player_count) {
            if i == winner {
                continue;
            }
            if i == dealer_idx {
                *delta = -dealer_pay;
                total_gain += dealer_pay;
            } else {
                *delta = -non_dealer_pay;
                total_gain += non_dealer_pay;
            }
        }
        deltas[winner] = total_gain;
    }

    deltas
}

/// Computes tsumo transfers when each entry in `pao_players` is liable for
/// one yakuman in a multi-yakuman hand.
///
/// Non-liable yakuman portions keep their normal split. The final liability
/// record covers the continuance bonus, preserving the existing single-pao
/// convention. If the score is not a yakuman, the sole liability record is
/// treated as covering the full hand.
#[allow(clippy::too_many_arguments)]
pub fn calculate_tsumo_score_deltas_with_pao(
    winner: usize,
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    dealer_idx: usize,
    honba: usize,
    player_count: usize,
    pao_players: &[usize],
) -> [i32; 4] {
    if pao_players.is_empty() {
        return calculate_tsumo_score_deltas(
            winner,
            score_result,
            winner_is_dealer,
            dealer_idx,
            honba,
            player_count,
        );
    }

    let yakuman_units = score_result
        .yaku_list
        .iter()
        .filter_map(|(_, han)| (*han >= 13).then_some(*han / 13))
        .sum::<u32>() as usize;

    if yakuman_units == 0 {
        let mut deltas = calculate_tsumo_score_deltas(
            winner,
            score_result,
            winner_is_dealer,
            dealer_idx,
            honba,
            player_count,
        );
        apply_pao_to_tsumo_deltas(&mut deltas, winner, pao_players[0]);
        return deltas;
    }

    let covered_units = pao_players.len().min(yakuman_units);
    let liable_players = &pao_players[..covered_units];
    let ordinary_units = yakuman_units - covered_units;
    let unit_divisor = yakuman_units as i32;
    let dealer_unit = score_result.dealer_tsumo_all as i32 / unit_divisor;
    let non_dealer_dealer_unit = score_result.non_dealer_tsumo_dealer as i32 / unit_divisor;
    let non_dealer_unit = score_result.non_dealer_tsumo_non_dealer as i32 / unit_divisor;

    let mut deltas = calculate_tsumo_deltas_from_payments(
        winner,
        dealer_unit * ordinary_units as i32,
        non_dealer_dealer_unit * ordinary_units as i32,
        non_dealer_unit * ordinary_units as i32,
        winner_is_dealer,
        dealer_idx,
        0,
        player_count,
    );

    for &pao_player in liable_players {
        let mut unit_deltas = calculate_tsumo_deltas_from_payments(
            winner,
            dealer_unit,
            non_dealer_dealer_unit,
            non_dealer_unit,
            winner_is_dealer,
            dealer_idx,
            0,
            player_count,
        );
        apply_pao_to_tsumo_deltas(&mut unit_deltas, winner, pao_player);
        for (delta, unit_delta) in deltas.iter_mut().zip(unit_deltas) {
            *delta += unit_delta;
        }
    }

    let honba_payment = honba as i32 * 100 * (player_count as i32 - 1);
    deltas[winner] += honba_payment;
    if let Some(&honba_payer) = liable_players.last() {
        deltas[honba_payer] -= honba_payment;
    }
    deltas
}

/// Applies a liability payment (pao / 包) to tsumo deltas.
///
/// The liable player covers every other player's payment. The total —
/// including honba bonuses and any tsumo-loss shortfall — moves to the
/// liable player as-is. `pao_player` must differ from `winner`.
pub fn apply_pao_to_tsumo_deltas(deltas: &mut [i32; 4], winner: usize, pao_player: usize) {
    let mut total_payment = 0i32;
    for (i, delta) in deltas.iter_mut().enumerate() {
        if i != winner {
            total_payment += *delta;
            *delta = 0;
        }
    }
    deltas[pao_player] = total_payment;
}

/// Computes the point transfers for a ron win with a liability payment.
///
/// The liable player and the deal-in player split the win; the honba
/// bonus is paid by the deal-in player. When the deal-in player is the
/// liable player, this reduces to an ordinary ron (full payment).
///
/// Returns each player's score delta; deltas always sum to zero.
pub fn calculate_ron_score_deltas_with_pao(
    winner: usize,
    loser: usize,
    pao_player: usize,
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    honba: usize,
) -> [i32; 4] {
    calculate_ron_score_deltas_with_pao_players(
        winner,
        loser,
        &[pao_player],
        score_result,
        winner_is_dealer,
        honba,
    )
}

/// Computes ron transfers when each entry in `pao_players` is liable for one
/// yakuman in a multi-yakuman hand. Other yakuman portions remain the deal-in
/// player's responsibility.
pub fn calculate_ron_score_deltas_with_pao_players(
    winner: usize,
    loser: usize,
    pao_players: &[usize],
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    honba: usize,
) -> [i32; 4] {
    if pao_players.is_empty() {
        return calculate_ron_score_deltas(winner, loser, score_result, winner_is_dealer, honba);
    }

    let yakuman_units = score_result
        .yaku_list
        .iter()
        .filter_map(|(_, han)| (*han >= 13).then_some(*han / 13))
        .sum::<u32>() as usize;
    if yakuman_units == 0 {
        return calculate_ron_deltas_with_pao_points(
            winner,
            loser,
            pao_players[0],
            ron_points(score_result, winner_is_dealer),
            honba,
        );
    }

    let covered_units = pao_players.len().min(yakuman_units);
    let liable_players = &pao_players[..covered_units];
    let ordinary_units = yakuman_units - covered_units;
    let unit_points = ron_points(score_result, winner_is_dealer) / yakuman_units as i32;
    let mut deltas =
        calculate_ron_deltas_from_points(winner, loser, unit_points * ordinary_units as i32, 0);

    for &pao_player in liable_players {
        let unit_deltas =
            calculate_ron_deltas_with_pao_points(winner, loser, pao_player, unit_points, 0);
        for (delta, unit_delta) in deltas.iter_mut().zip(unit_deltas) {
            *delta += unit_delta;
        }
    }

    let honba_bonus = honba as i32 * 300;
    deltas[winner] += honba_bonus;
    deltas[loser] -= honba_bonus;
    deltas
}

fn calculate_ron_deltas_with_pao_points(
    winner: usize,
    loser: usize,
    pao_player: usize,
    ron_points: i32,
    honba: usize,
) -> [i32; 4] {
    if pao_player == loser {
        return calculate_ron_deltas_from_points(winner, loser, ron_points, honba);
    }

    let mut deltas = [0i32; 4];
    let honba_bonus = honba as i32 * 300;

    // Split; any odd remainder goes to the deal-in player.
    let pao_half = ron_points / 2;
    let loser_half = ron_points - pao_half;

    deltas[winner] = ron_points + honba_bonus;
    deltas[loser] = -(loser_half + honba_bonus);
    deltas[pao_player] = -pao_half;

    deltas
}

/// Adds dora han (dora, red fives, ura dora, pei dora) to a score and
/// recomputes the han, rank, and payments.
///
/// * `extra_tile` - the winning tile on a ron, passed separately because
///   it is not part of the hand
/// * `uradora_indicators` - non-empty only on a riichi win
/// * `pei_tiles` - extracted North tiles (three-player only); each is
///   worth one han, and also counts again as indicator dora when an
///   indicator points at North
/// * `three_player` - wraps the characters dora chain 1m<->9m
pub fn add_dora_to_score(
    score_result: &mut ScoreResult,
    hand: &Hand,
    extra_tile: Option<Tile>,
    dora_indicators: &[Tile],
    uradora_indicators: &[Tile],
    pei_tiles: &[Tile],
    three_player: bool,
) {
    // Yakuman hands score a fixed amount; dora never applies.
    if score_result.yaku_list.iter().any(|(_, h)| *h >= 13) {
        return;
    }

    // Collect every tile of the winning hand; extracted North tiles also
    // count towards indicator dora.
    let mut all_tiles: Vec<Tile> = hand.tiles().to_vec();
    if let Some(drawn) = hand.drawn() {
        all_tiles.push(drawn);
    }
    if let Some(tile) = extra_tile {
        all_tiles.push(tile);
    }
    for open in hand.melds() {
        all_tiles.extend(open.expanded_tiles());
    }
    all_tiles.extend_from_slice(pei_tiles);

    let mut dora_count: u32 = 0;
    for indicator in dora_indicators {
        let dora_type = dora_indicator_to_dora_in(indicator.get(), three_player);
        dora_count += all_tiles.iter().filter(|t| t.get() == dora_type).count() as u32;
    }

    let mut uradora_count: u32 = 0;
    for indicator in uradora_indicators {
        let dora_type = dora_indicator_to_dora_in(indicator.get(), three_player);
        uradora_count += all_tiles.iter().filter(|t| t.get() == dora_type).count() as u32;
    }

    let red_dora_count = all_tiles.iter().filter(|t| t.is_red_dora()).count() as u32;

    let pei_count = pei_tiles.len() as u32;

    let extra_han = dora_count + uradora_count + red_dora_count + pei_count;
    if extra_han == 0 {
        return;
    }

    let new_han = score_result.han + extra_han;
    score_result.han = new_han;

    score_result.rank = determine_rank(new_han, score_result.fu, false);
    let base_points = calculate_base_points(new_han, score_result.fu, score_result.rank);
    score_result.dealer_ron = round_up_to_100(base_points * 6);
    score_result.dealer_tsumo_all = round_up_to_100(base_points * 2);
    score_result.non_dealer_ron = round_up_to_100(base_points * 4);
    score_result.non_dealer_tsumo_dealer = round_up_to_100(base_points * 2);
    score_result.non_dealer_tsumo_non_dealer = round_up_to_100(base_points);

    // Appended in display order: dora, red five, ura dora, pei dora.
    if dora_count > 0 {
        score_result
            .yaku_list
            .push((ScoreItem::Dora(DoraLabel::Dora), dora_count));
    }
    if red_dora_count > 0 {
        score_result
            .yaku_list
            .push((ScoreItem::Dora(DoraLabel::RedDora), red_dora_count));
    }
    if uradora_count > 0 {
        score_result
            .yaku_list
            .push((ScoreItem::Dora(DoraLabel::UraDora), uradora_count));
    }
    if pei_count > 0 {
        score_result
            .yaku_list
            .push((ScoreItem::Dora(DoraLabel::PeiDora), pei_count));
    }
}

/// Whether the player's 13-tile hand is tenpai.
pub fn is_ready(player: &Player) -> bool {
    hand_analyzer::calc_shanten_number(&player.hand).is_ready()
}

/// Computes the point transfers for a ron win.
///
/// Returns each player's score delta; deltas always sum to zero.
pub fn calculate_ron_score_deltas(
    winner: usize,
    loser: usize,
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    honba: usize,
) -> [i32; 4] {
    calculate_ron_deltas_from_points(
        winner,
        loser,
        ron_points(score_result, winner_is_dealer),
        honba,
    )
}

fn ron_points(score_result: &ScoreResult, winner_is_dealer: bool) -> i32 {
    if winner_is_dealer {
        score_result.dealer_ron as i32
    } else {
        score_result.non_dealer_ron as i32
    }
}

fn calculate_ron_deltas_from_points(
    winner: usize,
    loser: usize,
    ron_points: i32,
    honba: usize,
) -> [i32; 4] {
    let mut deltas = [0i32; 4];
    let honba_bonus = honba as i32 * 300;

    deltas[winner] = ron_points + honba_bonus;
    deltas[loser] = -(ron_points + honba_bonus);

    deltas
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::hand::Hand;
    use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
    use mahjong_core::scoring::fu::{FuDetail, FuResult};
    use mahjong_core::scoring::score::{DoraLabel, ScoreItem, ScoreRank};
    use mahjong_core::tile::Tile;
    use mahjong_core::winning_hand::name::Kind;

    fn make_mangan_score() -> ScoreResult {
        ScoreResult {
            han: 5,
            fu: 30,
            rank: ScoreRank::Mangan,
            dealer_ron: 12000,
            dealer_tsumo_all: 4000,
            non_dealer_ron: 8000,
            non_dealer_tsumo_dealer: 4000,
            non_dealer_tsumo_non_dealer: 2000,
            yaku_list: vec![],
            has_opened: false,
            fu_result: FuResult {
                total: 30,
                details: vec![FuDetail {
                    name: "副底",
                    fu: 20,
                }],
            },
        }
    }

    #[test]
    fn test_tsumo_dealer_mangan() {
        let score = make_mangan_score();
        let deltas = calculate_tsumo_score_deltas(0, &score, true, 0, 0, 4);
        assert_eq!(deltas[0], 12000); // 4000 * 3
        assert_eq!(deltas[1], -4000);
        assert_eq!(deltas[2], -4000);
        assert_eq!(deltas[3], -4000);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_tsumo_non_dealer_mangan() {
        let score = make_mangan_score();
        let deltas = calculate_tsumo_score_deltas(1, &score, false, 0, 0, 4);
        assert_eq!(deltas[0], -4000); // dealer
        assert_eq!(deltas[1], 8000); // winner: 4000+2000+2000
        assert_eq!(deltas[2], -2000);
        assert_eq!(deltas[3], -2000);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_tsumo_with_honba() {
        let score = make_mangan_score();
        // Two honba add 100 x 2 = 200 to each payment.
        let deltas = calculate_tsumo_score_deltas(0, &score, true, 0, 2, 4);
        assert_eq!(deltas[1], -4200); // 4000+200
        assert_eq!(deltas[2], -4200);
        assert_eq!(deltas[3], -4200);
        assert_eq!(deltas[0], 12600); // 4200*3
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_sanma_tsumo_dealer_mangan_tsumo_loss() {
        let score = make_mangan_score();
        let deltas = calculate_tsumo_score_deltas(0, &score, true, 0, 0, 3);
        // Tsumo loss: payments stay 4000 each; the absent player's
        // share is simply not received.
        assert_eq!(deltas[0], 8000); // 4000 * 2
        assert_eq!(deltas[1], -4000);
        assert_eq!(deltas[2], -4000);
        assert_eq!(deltas[3], 0); // The dummy seat never pays.
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_sanma_tsumo_non_dealer_mangan_tsumo_loss() {
        let score = make_mangan_score();
        let deltas = calculate_tsumo_score_deltas(1, &score, false, 0, 0, 3);
        assert_eq!(deltas[0], -4000); // dealer
        assert_eq!(deltas[1], 6000); // 4000 + 2000 (tsumo loss)
        assert_eq!(deltas[2], -2000);
        assert_eq!(deltas[3], 0); // The dummy seat never pays.
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_sanma_dora_wraps_manzu_indicator() {
        let mut score = ScoreResult {
            han: 1,
            fu: 30,
            rank: ScoreRank::Normal,
            dealer_ron: 1500,
            dealer_tsumo_all: 500,
            non_dealer_ron: 1000,
            non_dealer_tsumo_dealer: 500,
            non_dealer_tsumo_non_dealer: 300,
            yaku_list: vec![(ScoreItem::Yaku(Kind::AllInside), 1)],
            has_opened: false,
            fu_result: FuResult {
                total: 30,
                details: vec![FuDetail {
                    name: "副底",
                    fu: 20,
                }],
            },
        };
        let tiles = vec![
            Tile::new(Tile::M9),
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::P4),
            Tile::new(Tile::S5),
            Tile::new(Tile::S6),
        ];
        let mut player = Player::new(Wind::South, tiles, 35000);
        player.draw(Tile::new(Tile::S7));

        // Three-player: a 1m indicator wraps to 9m as the dora.
        let dora_indicators = vec![Tile::new(Tile::M1)];
        add_dora_to_score(
            &mut score,
            &player.hand,
            None,
            &dora_indicators,
            &[],
            &[],
            true,
        );

        assert!(
            score
                .yaku_list
                .contains(&(ScoreItem::Dora(DoraLabel::Dora), 1)),
            "三麻の1m表示で9mがドラとしてカウントされない: {:?}",
            score.yaku_list
        );
    }

    #[test]
    fn test_pei_dora_added_and_double_counted_with_north_indicator() {
        let mut score = ScoreResult {
            han: 1,
            fu: 30,
            rank: ScoreRank::Normal,
            dealer_ron: 1500,
            dealer_tsumo_all: 500,
            non_dealer_ron: 1000,
            non_dealer_tsumo_dealer: 500,
            non_dealer_tsumo_non_dealer: 300,
            yaku_list: vec![(ScoreItem::Yaku(Kind::AllInside), 1)],
            has_opened: false,
            fu_result: FuResult {
                total: 30,
                details: vec![FuDetail {
                    name: "副底",
                    fu: 20,
                }],
            },
        };
        let tiles = vec![
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::P4),
            Tile::new(Tile::S5),
            Tile::new(Tile::S6),
            Tile::new(Tile::S7),
        ];
        let mut player = Player::new(Wind::South, tiles, 35000);
        player.draw(Tile::new(Tile::S8));

        // Two extracted Norths. A West (3z) indicator makes North (4z)
        // the dora, so they count as 2 pei-dora han plus 2 indicator-dora
        // han: 4 in total.
        let pei_tiles = vec![Tile::new(Tile::Z4), Tile::new(Tile::Z4)];
        let dora_indicators = vec![Tile::new(Tile::Z3)];
        add_dora_to_score(
            &mut score,
            &player.hand,
            None,
            &dora_indicators,
            &[],
            &pei_tiles,
            true,
        );

        assert!(
            score
                .yaku_list
                .contains(&(ScoreItem::Dora(DoraLabel::PeiDora), 2)),
            "北ドラ2翻が加算されていない: {:?}",
            score.yaku_list
        );
        assert!(
            score
                .yaku_list
                .contains(&(ScoreItem::Dora(DoraLabel::Dora), 2)),
            "西表示牌による北のドラ2翻が加算されていない: {:?}",
            score.yaku_list
        );
        // 1 (All Inside) + 2 (pei dora) + 2 (indicator dora) = 5 han
        assert_eq!(score.han, 5);
    }

    fn make_yakuman_score() -> ScoreResult {
        ScoreResult {
            han: 13,
            fu: 0,
            rank: ScoreRank::Yakuman,
            dealer_ron: 48000,
            dealer_tsumo_all: 16000,
            non_dealer_ron: 32000,
            non_dealer_tsumo_dealer: 16000,
            non_dealer_tsumo_non_dealer: 8000,
            yaku_list: vec![(ScoreItem::Yaku(Kind::BigDragons), 13)],
            has_opened: true,
            fu_result: FuResult {
                total: 0,
                details: vec![],
            },
        }
    }

    fn make_double_yakuman_score() -> ScoreResult {
        let mut score = make_yakuman_score();
        score.han = 26;
        score.dealer_ron *= 2;
        score.dealer_tsumo_all *= 2;
        score.non_dealer_ron *= 2;
        score.non_dealer_tsumo_dealer *= 2;
        score.non_dealer_tsumo_non_dealer *= 2;
        score
            .yaku_list
            .push((ScoreItem::Yaku(Kind::AllHonours), 13));
        score
    }

    /// Tsumo with pao: the liable player pays everything.
    #[test]
    fn test_pao_tsumo_non_dealer_yakuman() {
        let score = make_yakuman_score();
        let mut deltas = calculate_tsumo_score_deltas(1, &score, false, 0, 0, 4);
        apply_pao_to_tsumo_deltas(&mut deltas, 1, 2);
        assert_eq!(deltas[0], 0);
        assert_eq!(deltas[1], 32000); // 16000 + 8000 + 8000
        assert_eq!(deltas[2], -32000); // The liable player pays everything.
        assert_eq!(deltas[3], 0);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_pao_tsumo_only_shifts_the_liable_yakuman_portion() {
        let score = make_double_yakuman_score();
        let deltas = calculate_tsumo_score_deltas_with_pao(1, &score, false, 0, 0, 4, &[2]);

        // Big Dragons is paid entirely by seat 2; All Honours keeps its
        // ordinary tsumo split.
        assert_eq!(deltas, [-16000, 64000, -40000, -8000]);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    /// Tsumo with pao and honba: the liable player pays the honba
    /// bonus too.
    #[test]
    fn test_pao_tsumo_dealer_yakuman_with_honba() {
        let score = make_yakuman_score();
        let mut deltas = calculate_tsumo_score_deltas(0, &score, true, 0, 2, 4);
        apply_pao_to_tsumo_deltas(&mut deltas, 0, 3);
        assert_eq!(deltas[0], 48600); // (16000+200) * 3
        assert_eq!(deltas[1], 0);
        assert_eq!(deltas[2], 0);
        assert_eq!(deltas[3], -48600);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    /// Three-player pao tsumo: the tsumo-loss shortfall remains;
    /// only the actual total is covered.
    #[test]
    fn test_pao_tsumo_sanma_tsumo_loss() {
        let score = make_yakuman_score();
        let mut deltas = calculate_tsumo_score_deltas(1, &score, false, 0, 0, 3);
        apply_pao_to_tsumo_deltas(&mut deltas, 1, 2);
        assert_eq!(deltas[0], 0);
        assert_eq!(deltas[1], 24000); // 16000 + 8000 (tsumo loss)
        assert_eq!(deltas[2], -24000);
        assert_eq!(deltas[3], 0);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    /// Ron with pao where a third player dealt in: the deal-in player
    /// and the liable player split the payment.
    #[test]
    fn test_pao_ron_split_between_loser_and_pao() {
        let score = make_yakuman_score();
        let deltas = calculate_ron_score_deltas_with_pao(1, 3, 0, &score, false, 0);
        assert_eq!(deltas[1], 32000);
        assert_eq!(deltas[3], -16000); // deal-in player's half
        assert_eq!(deltas[0], -16000); // liable player's half
        assert_eq!(deltas[2], 0);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_pao_ron_only_splits_the_liable_yakuman_portion() {
        let score = make_double_yakuman_score();
        let deltas = calculate_ron_score_deltas_with_pao(1, 3, 2, &score, false, 0);

        // The deal-in player pays all of All Honours and half of Big
        // Dragons; the liable player pays the other Big Dragons half.
        assert_eq!(deltas, [0, 64000, -16000, -48000]);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    /// Ron with pao and honba: the deal-in player pays the honba bonus.
    #[test]
    fn test_pao_ron_honba_paid_by_loser() {
        let score = make_yakuman_score();
        let deltas = calculate_ron_score_deltas_with_pao(1, 3, 0, &score, false, 2);
        assert_eq!(deltas[1], 32600);
        assert_eq!(deltas[3], -16600); // half + honba 600
        assert_eq!(deltas[0], -16000);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    /// Deal-in by the liable player: same as an ordinary ron
    /// (full payment).
    #[test]
    fn test_pao_ron_from_pao_player_pays_full() {
        let score = make_yakuman_score();
        let deltas = calculate_ron_score_deltas_with_pao(1, 0, 0, &score, false, 1);
        assert_eq!(deltas[1], 32300);
        assert_eq!(deltas[0], -32300);
        assert_eq!(deltas[2], 0);
        assert_eq!(deltas[3], 0);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_ron_dealer_mangan() {
        let score = make_mangan_score();
        let deltas = calculate_ron_score_deltas(0, 2, &score, true, 0);
        assert_eq!(deltas[0], 12000);
        assert_eq!(deltas[2], -12000);
        assert_eq!(deltas[1], 0);
        assert_eq!(deltas[3], 0);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_ron_non_dealer_mangan() {
        let score = make_mangan_score();
        let deltas = calculate_ron_score_deltas(1, 3, &score, false, 0);
        assert_eq!(deltas[1], 8000);
        assert_eq!(deltas[3], -8000);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_ron_with_honba() {
        let score = make_mangan_score();
        // Three honba add 300 x 3 = 900.
        let deltas = calculate_ron_score_deltas(1, 3, &score, false, 3);
        assert_eq!(deltas[1], 8900);
        assert_eq!(deltas[3], -8900);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_check_win_non_winning_hand() {
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let mut player = Player::new(Wind::East, tiles, 25000);
        player.draw(Tile::new(Tile::Z5));

        let result = check_win(&player, Wind::East, true, false, false);
        assert!(!result.is_win);
        assert!(result.score_result.is_none());
    }

    #[test]
    fn test_check_nagashi_mangan_scores_a_non_winning_shape() {
        let hand = Hand::from("147m258p369s1234z");
        let player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);

        let result = check_nagashi_mangan(&player, Wind::East, &Settings::new());

        assert!(result.is_win);
        let score = result.score_result.expect("Nagashi Mangan score");
        assert_eq!(score.han, 5);
        assert_eq!(score.fu, 30);
        assert_eq!(score.rank, ScoreRank::Mangan);
        assert_eq!(
            score.yaku_list,
            vec![(ScoreItem::Yaku(Kind::NagashiMangan), 5)]
        );
        assert_eq!(score.non_dealer_ron, 8000);
    }

    #[test]
    fn test_check_nagashi_mangan_scores_an_open_hand() {
        use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

        let hand = Hand::from("147m258p369s1z");
        let mut player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);
        player.hand.add_meld(Meld {
            tiles: vec![Tile::new(Tile::Z5); 3],
            category: MeldType::Pon,
            from: MeldFrom::Previous,
            called_tile: Some(Tile::new(Tile::Z5)),
        });

        let result = check_nagashi_mangan(&player, Wind::East, &Settings::new());

        assert!(result.is_win);
        let score = result.score_result.expect("open Nagashi Mangan score");
        assert_eq!(score.rank, ScoreRank::Mangan);
        assert!(score.has_opened);
    }

    #[test]
    fn test_check_win_tsumo() {
        let hand = Hand::from("123m456p789s1112z 2z");
        let tiles: Vec<Tile> = hand.tiles().to_vec();
        let drawn = hand.drawn();
        let mut player = Player::new(Wind::South, tiles, 25000);
        if let Some(d) = drawn {
            player.draw(d);
        }

        let result = check_win(&player, Wind::East, true, false, false);
        assert!(result.is_win);
        let score = result.score_result.unwrap();
        // Fully Concealed Hand (1) + round wind (1) = 2 han
        assert!(score.han >= 2);
    }

    #[test]
    fn test_check_win_closed_tsumo_with_iipeikou_shape() {
        let hand = Hand::from("2256678m234p456s 7m");
        let tiles: Vec<Tile> = hand.tiles().to_vec();
        let drawn = hand.drawn();
        let mut player = Player::new(Wind::East, tiles, 25000);
        if let Some(d) = drawn {
            player.draw(d);
        }

        let result = check_win(&player, Wind::East, true, false, false);
        assert!(result.is_win, "closed tsumo hand should be a win");
        let score = result.score_result.unwrap();
        assert!(score.han >= 1, "expected at least menzen tsumo");
    }

    #[test]
    fn test_check_win_open_tanyao_tsumo() {
        let hand = Hand::from("56677m66s 5m");
        let tiles: Vec<Tile> = hand.tiles().to_vec();
        let drawn = hand.drawn();
        let mut player = Player::new(Wind::South, tiles, 25000);
        player.hand.add_meld(Meld {
            tiles: vec![
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: None,
        });
        player.hand.add_meld(Meld {
            tiles: vec![
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::M4),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: None,
        });
        if let Some(d) = drawn {
            player.draw(d);
        }

        let result = check_win(&player, Wind::East, true, false, false);
        assert!(result.is_win, "open tanyao tsumo should be a win");
        let score = result.score_result.unwrap();
        assert!(score.han >= 1, "expected at least tanyao");
    }

    #[test]
    fn test_check_win_respects_open_tanyao_disabled() {
        let hand = Hand::from("56677m66s 5m");
        let tiles: Vec<Tile> = hand.tiles().to_vec();
        let drawn = hand.drawn();
        let mut player = Player::new(Wind::South, tiles, 25000);
        player.hand.add_meld(Meld {
            tiles: vec![
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: None,
        });
        player.hand.add_meld(Meld {
            tiles: vec![
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::M4),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: None,
        });
        if let Some(d) = drawn {
            player.draw(d);
        }

        let mut settings = Settings::new();
        settings.opened_all_inside = false;

        let result = check_win_with_settings(&player, Wind::East, true, false, false, &settings);
        assert!(!result.is_win, "open tanyao must be rejected when disabled");
    }

    #[test]
    fn test_check_ron_rejects_four_melds_and_one_taatsu() {
        let hand = Hand::from("234678m56p567s55z");
        let player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);

        let result = check_ron(&player, Tile::new(Tile::Z5), Wind::East, false);
        assert!(!result.is_win);
        assert!(result.score_result.is_none());

        assert!(player.can_pon(Tile::new(Tile::Z5)));
    }

    #[test]
    fn test_get_waiting_tiles_for_47p_shape() {
        let hand = Hand::from("234678m56p567s55z");
        let player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);

        let waiting = get_waiting_tiles(&player);
        assert_eq!(waiting, vec![Tile::P4, Tile::P7]);
    }

    /// Dora entries must follow the yaku in the order
    /// dora, red five, ura dora.
    #[test]
    fn test_dora_order_in_yaku_list() {
        use mahjong_core::tile::Wind;

        let fu_result = FuResult {
            total: 30,
            details: vec![FuDetail {
                name: "副底",
                fu: 20,
            }],
        };
        let mut score = ScoreResult {
            han: 1,
            fu: 30,
            rank: ScoreRank::Normal,
            dealer_ron: 1500,
            dealer_tsumo_all: 500,
            non_dealer_ron: 1000,
            non_dealer_tsumo_dealer: 500,
            non_dealer_tsumo_non_dealer: 300,
            yaku_list: vec![(ScoreItem::Yaku(Kind::AllInside), 1)],
            has_opened: false,
            fu_result,
        };

        let tiles = vec![
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
            Tile::new(Tile::S7),
        ];
        let mut player = Player::new(Wind::South, tiles, 25000);
        player.draw(Tile::new_red(Tile::M5));

        // Indicator 1m -> dora 2m; ura indicator 6s -> ura dora 7s;
        // plus the red 5m: one of each.
        let dora_indicators = vec![Tile::new(Tile::M1)];
        let uradora_indicators = vec![Tile::new(Tile::S6)];

        add_dora_to_score(
            &mut score,
            &player.hand,
            None,
            &dora_indicators,
            &uradora_indicators,
            &[],
            false,
        );

        assert_eq!(score.yaku_list.len(), 4);
        assert_eq!(score.yaku_list[0], (ScoreItem::Yaku(Kind::AllInside), 1));
        assert_eq!(score.yaku_list[1], (ScoreItem::Dora(DoraLabel::Dora), 1));
        assert_eq!(score.yaku_list[2], (ScoreItem::Dora(DoraLabel::RedDora), 1));
        assert_eq!(score.yaku_list[3], (ScoreItem::Dora(DoraLabel::UraDora), 1));
    }

    #[test]
    fn test_add_dora_counts_red_called_kan_tile() {
        let fu_result = FuResult {
            total: 30,
            details: vec![FuDetail {
                name: "副底",
                fu: 20,
            }],
        };
        let mut score = ScoreResult {
            han: 1,
            fu: 30,
            rank: ScoreRank::Normal,
            dealer_ron: 1500,
            dealer_tsumo_all: 500,
            non_dealer_ron: 1000,
            non_dealer_tsumo_dealer: 500,
            non_dealer_tsumo_non_dealer: 300,
            yaku_list: vec![(ScoreItem::Yaku(Kind::Riichi), 1)],
            has_opened: false,
            fu_result,
        };
        let hand = Hand::new_with_melds(
            vec![],
            vec![Meld {
                tiles: vec![Tile::new(Tile::M5); 3],
                category: MeldType::Kan,
                from: MeldFrom::Previous,
                called_tile: Some(Tile::new_red(Tile::M5)),
            }],
            None,
        );

        add_dora_to_score(&mut score, &hand, None, &[], &[], &[], false);

        assert_eq!(
            score.yaku_list.last(),
            Some(&(ScoreItem::Dora(DoraLabel::RedDora), 1))
        );
        assert_eq!(score.han, 2);
    }

    #[test]
    fn test_add_dora_counts_red_closed_kan_once() {
        let fu_result = FuResult {
            total: 30,
            details: vec![FuDetail {
                name: "副底",
                fu: 20,
            }],
        };
        let mut score = ScoreResult {
            han: 1,
            fu: 30,
            rank: ScoreRank::Normal,
            dealer_ron: 1500,
            dealer_tsumo_all: 500,
            non_dealer_ron: 1000,
            non_dealer_tsumo_dealer: 500,
            non_dealer_tsumo_non_dealer: 300,
            yaku_list: vec![(ScoreItem::Yaku(Kind::Riichi), 1)],
            has_opened: false,
            fu_result,
        };
        let hand = Hand::new_with_melds(
            vec![],
            vec![Meld {
                tiles: vec![
                    Tile::new_red(Tile::M5),
                    Tile::new(Tile::M5),
                    Tile::new(Tile::M5),
                ],
                category: MeldType::Kan,
                from: MeldFrom::Myself,
                called_tile: None,
            }],
            None,
        );

        add_dora_to_score(&mut score, &hand, None, &[], &[], &[], false);

        assert_eq!(
            score.yaku_list.last(),
            Some(&(ScoreItem::Dora(DoraLabel::RedDora), 1))
        );
        assert_eq!(score.han, 2);
    }
}
