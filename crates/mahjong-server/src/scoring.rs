//! 得点計算（サーバ側）
//!
//! mahjong-core の得点計算機能をサーバ側から呼び出すラッパー。
//! プレイヤーの手牌と局の状態から和了判定・点数計算を行い、
//! 点数移動を適用する。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{self, HandAnalyzer};
use mahjong_core::hand_info::meld::Meld;
use mahjong_core::hand_info::status::Status;
use mahjong_core::scoring::score::{
    ScoreRank, ScoreResult, calculate_base_points, calculate_score, determine_rank, round_up_to_100,
};
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora};

use crate::player::Player;

/// 和了判定の結果
#[derive(Debug)]
pub struct WinCheckResult {
    /// 和了が成立するか
    pub is_win: bool,
    /// 点数計算の結果（和了が成立する場合のみSome）
    pub score_result: Option<ScoreResult>,
}

/// プレイヤーの手牌が和了しているか判定する
///
/// ツモ和了の場合: `is_tsumo = true`
/// ロン和了の場合: `is_tsumo = false`
pub fn check_win(
    player: &Player,
    prevailing_wind: Wind,
    is_tsumo: bool,
    is_last_tile: bool,
    is_dead_wall_draw: bool,
) -> WinCheckResult {
    let settings = Settings::new();
    check_win_with_settings(
        player,
        prevailing_wind,
        is_tsumo,
        is_last_tile,
        is_dead_wall_draw,
        &settings,
    )
}

/// プレイヤーの手牌が和了しているか、指定ルールで判定する
pub fn check_win_with_settings(
    player: &Player,
    prevailing_wind: Wind,
    is_tsumo: bool,
    is_last_tile: bool,
    is_dead_wall_draw: bool,
    settings: &Settings,
) -> WinCheckResult {
    let hand = &player.hand;

    // HandAnalyzer で向聴数を計算（ツモ牌込み）
    let analyzer = match HandAnalyzer::new(hand) {
        Ok(a) => a,
        Err(_) => {
            return WinCheckResult {
                is_win: false,
                score_result: None,
            };
        }
    };

    // 和了形（shanten == -1）でなければ不成立
    if !analyzer.shanten.has_won() {
        return WinCheckResult {
            is_win: false,
            score_result: None,
        };
    }

    // Status を構築
    let mut status = Status::new();
    status.is_self_picked = is_tsumo;
    status.player_wind = player.seat_wind;
    status.prevailing_wind = prevailing_wind;
    status.has_claimed_ready = player.is_riichi;
    status.is_double_ready = player.is_double_riichi;
    status.is_one_shot = player.is_ippatsu;
    status.has_claimed_open = !player.is_menzen();
    status.is_dealer = player.is_dealer();
    status.is_first_turn = player.is_first_turn;
    status.is_last_tile_from_the_wall = is_last_tile && is_tsumo;
    status.is_last_discard = is_last_tile && !is_tsumo;
    status.is_dead_wall_draw = is_dead_wall_draw;
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

/// ロン和了が可能か判定する
///
/// プレイヤーの手牌（13枚）に対して、捨て牌を加えてロン和了になるか判定する。
/// フリテン判定は呼び出し元で行うこと。
pub fn check_ron(
    player: &Player,
    discarded_tile: Tile,
    prevailing_wind: Wind,
    is_last_tile: bool,
) -> WinCheckResult {
    let settings = Settings::new();
    check_ron_with_flags_and_settings(
        player,
        discarded_tile,
        prevailing_wind,
        is_last_tile,
        false,
        &settings,
    )
}

/// ロン和了が可能か指定ルールで判定する
pub fn check_ron_with_settings(
    player: &Player,
    discarded_tile: Tile,
    prevailing_wind: Wind,
    is_last_tile: bool,
    settings: &Settings,
) -> WinCheckResult {
    check_ron_with_flags_and_settings(
        player,
        discarded_tile,
        prevailing_wind,
        is_last_tile,
        false,
        settings,
    )
}

/// ロン和了が可能か判定する（搶槓などの状態フラグ付き）
pub fn check_ron_with_flags(
    player: &Player,
    discarded_tile: Tile,
    prevailing_wind: Wind,
    is_last_tile: bool,
    is_robbing_a_quad: bool,
) -> WinCheckResult {
    let settings = Settings::new();
    check_ron_with_flags_and_settings(
        player,
        discarded_tile,
        prevailing_wind,
        is_last_tile,
        is_robbing_a_quad,
        &settings,
    )
}

/// ロン和了が可能か指定ルールと状態フラグで判定する
pub fn check_ron_with_flags_and_settings(
    player: &Player,
    discarded_tile: Tile,
    prevailing_wind: Wind,
    is_last_tile: bool,
    is_robbing_a_quad: bool,
    settings: &Settings,
) -> WinCheckResult {
    // 手牌をクローンして捨て牌をdrawnとしてセット
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

    // Status を構築（ロンなので is_self_picked = false）
    let mut status = Status::new();
    status.is_self_picked = false;
    status.player_wind = player.seat_wind;
    status.prevailing_wind = prevailing_wind;
    status.has_claimed_ready = player.is_riichi;
    status.is_double_ready = player.is_double_riichi;
    status.is_one_shot = player.is_ippatsu;
    status.has_claimed_open = !player.is_menzen();
    status.is_dealer = player.is_dealer();
    status.is_first_turn = player.is_first_turn;
    status.is_last_tile_from_the_wall = false;
    status.is_last_discard = is_last_tile && !is_robbing_a_quad;
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

/// 聴牌している牌（待ち牌）の種類を取得する
///
/// フリテン判定に使用する。
/// 手牌が13枚（drawn=None）の状態で、各TileTypeを仮にdrawnにセットし、
/// 和了形（shanten == -1）になるものを全て返す。
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

/// ツモ和了の点数移動を計算する
///
/// - `winner`: 和了プレイヤーのインデックス (0-3)
/// - `score_result`: 点数計算の結果
/// - `winner_is_dealer`: 和了プレイヤーが親かどうか
/// - `dealer_idx`: 親のプレイヤーインデックス (0-3)
/// - `honba`: 本場数
///
/// 戻り値: 各プレイヤーの点数変動 (正=増加、負=減少)。合計は必ず0。
pub fn calculate_tsumo_score_deltas(
    winner: usize,
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    dealer_idx: usize,
    honba: usize,
) -> [i32; 4] {
    let mut deltas = [0i32; 4];
    let honba_bonus = honba as i32 * 100;

    if winner_is_dealer {
        // 親ツモ: 各子が dealer_tsumo_all + 本場ボーナス を支払う
        let each_pay = score_result.dealer_tsumo_all as i32 + honba_bonus;
        for (i, delta) in deltas.iter_mut().enumerate() {
            if i == winner {
                *delta = each_pay * 3;
            } else {
                *delta = -each_pay;
            }
        }
    } else {
        // 子ツモ: 親が non_dealer_tsumo_dealer、他の子が non_dealer_tsumo_non_dealer を支払う
        let dealer_pay = score_result.non_dealer_tsumo_dealer as i32 + honba_bonus;
        let non_dealer_pay = score_result.non_dealer_tsumo_non_dealer as i32 + honba_bonus;
        let mut total_gain = 0i32;
        for (i, delta) in deltas.iter_mut().enumerate() {
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

/// 点数等級を日本語文字列に変換する
pub fn rank_to_string(rank: &ScoreRank) -> &'static str {
    match rank {
        ScoreRank::Normal => "",
        ScoreRank::Mangan => "満貫",
        ScoreRank::Haneman => "跳満",
        ScoreRank::Baiman => "倍満",
        ScoreRank::Sanbaiman => "三倍満",
        ScoreRank::Yakuman => "役満",
    }
}

/// 和了結果にドラ・赤ドラ・裏ドラの翻を加算する
///
/// 役判定後の点数計算結果にドラ関連の翻を追加し、
/// 翻数・等級・支払い額を再計算する。
///
/// * `score_result` - 役判定後の点数計算結果（ドラ未加算）
/// * `hand` - 和了プレイヤーの手牌
/// * `extra_tile` - ロン和了の場合の和了牌（手牌に含まれていないため別途指定）
/// * `dora_indicators` - ドラ表示牌
/// * `uradora_indicators` - 裏ドラ表示牌（リーチ時のみ非空）
pub fn add_dora_to_score(
    score_result: &mut ScoreResult,
    hand: &Hand,
    extra_tile: Option<Tile>,
    dora_indicators: &[Tile],
    uradora_indicators: &[Tile],
) {
    // 役満の場合はドラを加算しない
    if score_result.yaku_list.iter().any(|(_, h)| *h >= 13) {
        return;
    }

    // 和了手牌の全牌を集める
    let mut all_tiles: Vec<Tile> = hand.tiles().to_vec();
    if let Some(drawn) = hand.drawn() {
        all_tiles.push(drawn);
    }
    if let Some(tile) = extra_tile {
        all_tiles.push(tile);
    }
    for open in hand.melds() {
        for &tile in &open.tiles {
            all_tiles.push(tile);
        }
        if open.category.is_kan() && open.tiles.len() == 3 {
            all_tiles.push(kan_fourth_tile(open));
        }
    }

    // ドラ表示牌からドラ牌を計算してカウント
    let mut dora_count: u32 = 0;
    for indicator in dora_indicators {
        let dora_type = dora_indicator_to_dora(indicator.get());
        dora_count += all_tiles.iter().filter(|t| t.get() == dora_type).count() as u32;
    }

    // 裏ドラ表示牌からドラ牌を計算してカウント
    let mut uradora_count: u32 = 0;
    for indicator in uradora_indicators {
        let dora_type = dora_indicator_to_dora(indicator.get());
        uradora_count += all_tiles.iter().filter(|t| t.get() == dora_type).count() as u32;
    }

    // 赤ドラをカウント
    let red_dora_count = all_tiles.iter().filter(|t| t.is_red_dora()).count() as u32;

    let extra_han = dora_count + uradora_count + red_dora_count;
    if extra_han == 0 {
        return;
    }

    // 翻数を再計算
    let new_han = score_result.han + extra_han;
    score_result.han = new_han;

    // 等級・点数を再計算
    score_result.rank = determine_rank(new_han, score_result.fu, false);
    let base_points = calculate_base_points(new_han, score_result.fu, score_result.rank);
    score_result.dealer_ron = round_up_to_100(base_points * 6);
    score_result.dealer_tsumo_all = round_up_to_100(base_points * 2);
    score_result.non_dealer_ron = round_up_to_100(base_points * 4);
    score_result.non_dealer_tsumo_dealer = round_up_to_100(base_points * 2);
    score_result.non_dealer_tsumo_non_dealer = round_up_to_100(base_points);

    // ドラ・赤ドラ・裏ドラをこの順で末尾に追加
    if dora_count > 0 {
        score_result.yaku_list.push(("ドラ", dora_count));
    }
    if red_dora_count > 0 {
        score_result.yaku_list.push(("赤ドラ", red_dora_count));
    }
    if uradora_count > 0 {
        score_result.yaku_list.push(("裏ドラ", uradora_count));
    }
}

fn kan_fourth_tile(open: &Meld) -> Tile {
    if let Some(tile) = open.called_tile {
        return tile;
    }

    open.tiles
        .iter()
        .copied()
        .find(|tile| !tile.is_red_dora())
        .unwrap_or(open.tiles[0])
}

/// プレイヤーがテンパイしているか判定する（13枚の手牌で）
pub fn is_ready(player: &Player) -> bool {
    hand_analyzer::calc_shanten_number(&player.hand).is_ready()
}

/// ロン和了の点数移動を計算する
///
/// - `winner`: 和了プレイヤーのインデックス (0-3)
/// - `loser`: 放銃プレイヤーのインデックス (0-3)
/// - `score_result`: 点数計算の結果
/// - `winner_is_dealer`: 和了プレイヤーが親かどうか
/// - `honba`: 本場数
///
/// 戻り値: 各プレイヤーの点数変動 (正=増加、負=減少)。合計は必ず0。
pub fn calculate_ron_score_deltas(
    winner: usize,
    loser: usize,
    score_result: &ScoreResult,
    winner_is_dealer: bool,
    honba: usize,
) -> [i32; 4] {
    let mut deltas = [0i32; 4];
    let honba_bonus = honba as i32 * 300; // ロンは本場1本場につき300点

    let ron_points = if winner_is_dealer {
        score_result.dealer_ron as i32
    } else {
        score_result.non_dealer_ron as i32
    };

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
    use mahjong_core::scoring::score::ScoreRank;
    use mahjong_core::tile::Tile;

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
        let deltas = calculate_tsumo_score_deltas(0, &score, true, 0, 0);
        assert_eq!(deltas[0], 12000); // 4000 * 3
        assert_eq!(deltas[1], -4000);
        assert_eq!(deltas[2], -4000);
        assert_eq!(deltas[3], -4000);
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_tsumo_non_dealer_mangan() {
        let score = make_mangan_score();
        let deltas = calculate_tsumo_score_deltas(1, &score, false, 0, 0);
        assert_eq!(deltas[0], -4000); // 親
        assert_eq!(deltas[1], 8000); // 和了者: 4000+2000+2000
        assert_eq!(deltas[2], -2000); // 子
        assert_eq!(deltas[3], -2000); // 子
        assert_eq!(deltas.iter().sum::<i32>(), 0);
    }

    #[test]
    fn test_tsumo_with_honba() {
        let score = make_mangan_score();
        // 2本場: 各プレイヤーの支払いに100*2=200点加算
        let deltas = calculate_tsumo_score_deltas(0, &score, true, 0, 2);
        assert_eq!(deltas[1], -4200); // 4000+200
        assert_eq!(deltas[2], -4200);
        assert_eq!(deltas[3], -4200);
        assert_eq!(deltas[0], 12600); // 4200*3
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
        // 3本場: 300*3=900点加算
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
    fn test_check_win_tsumo() {
        // 123m456p789s111z + 2zツモ = 門前ツモ + 場風(東)
        // 合計14枚: 123m(順子) + 456p(順子) + 789s(順子) + 111z(東刻子) + 22z(雀頭)
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
        // 門前ツモ(1翻) + 場風(1翻) = 2翻
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
        settings.opened_all_simples = false;

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

    /// 通常役の後にドラ→赤ドラ→裏ドラの順で並ぶことを確認する
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
            yaku_list: vec![("断么九", 1)],
            fu_result,
        };

        // 手牌にM2（ドラ）・赤M5（赤ドラ）・S7（裏ドラ対象）を含む
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
        player.draw(Tile::new_red(Tile::M5)); // 赤ドラ

        // ドラ表示牌M1 → ドラはM2（1枚）
        // 裏ドラ表示牌S6 → 裏ドラはS7（1枚）
        // 赤ドラ: 赤M5（1枚）
        let dora_indicators = vec![Tile::new(Tile::M1)];
        let uradora_indicators = vec![Tile::new(Tile::S6)];

        add_dora_to_score(
            &mut score,
            &player.hand,
            None,
            &dora_indicators,
            &uradora_indicators,
        );

        assert_eq!(score.yaku_list.len(), 4);
        assert_eq!(score.yaku_list[0], ("断么九", 1));
        assert_eq!(score.yaku_list[1], ("ドラ", 1));
        assert_eq!(score.yaku_list[2], ("赤ドラ", 1));
        assert_eq!(score.yaku_list[3], ("裏ドラ", 1));
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
            yaku_list: vec![("立直", 1)],
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

        add_dora_to_score(&mut score, &hand, None, &[], &[]);

        assert_eq!(score.yaku_list.last(), Some(&("赤ドラ", 1)));
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
            yaku_list: vec![("立直", 1)],
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

        add_dora_to_score(&mut score, &hand, None, &[], &[]);

        assert_eq!(score.yaku_list.last(), Some(&("赤ドラ", 1)));
        assert_eq!(score.han, 2);
    }
}
