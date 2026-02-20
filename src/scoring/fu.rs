use anyhow::Result;

use crate::hand::Hand;
use crate::hand_info::hand_analyzer::HandAnalyzer;
use crate::hand_info::opened::{OpenFrom, OpenType};
use crate::hand_info::status::Status;
use crate::tile::{Dragon, Tile, TileType, Wind};
use crate::winning_hand::name::Form;

/// 符計算の結果
#[derive(Debug, PartialEq, Eq)]
pub struct FuResult {
    /// 合計符（10符単位に切り上げ済み）
    pub total: u32,
    /// 符の内訳
    pub details: Vec<FuDetail>,
}

/// 符の内訳を表す構造体
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FuDetail {
    /// 符の名称
    pub name: &'static str,
    /// 符の値
    pub fu: u32,
}

/// 符を計算する
///
/// # Arguments
/// * `analyzer` - 手牌解析結果
/// * `hand` - 手牌
/// * `status` - 局の状態
///
/// # Returns
/// 符計算の結果（切り上げ済み合計 + 内訳）
pub fn calculate_fu(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
) -> Result<FuResult> {
    // 七対子は固定25符
    if analyzer.form == Form::SevenPairs {
        return Ok(FuResult {
            total: 25,
            details: vec![FuDetail {
                name: "七対子",
                fu: 25,
            }],
        });
    }

    // 国士無双は符計算なし（便宜上30符）
    if analyzer.form == Form::ThirteenOrphans {
        return Ok(FuResult {
            total: 30,
            details: vec![FuDetail {
                name: "国士無双",
                fu: 30,
            }],
        });
    }

    let mut details: Vec<FuDetail> = Vec::new();

    // 副底（基本符）：20符
    details.push(FuDetail {
        name: "副底",
        fu: 20,
    });

    // 面子の符
    calculate_mentsu_fu(analyzer, hand, status, &mut details)?;

    // 雀頭の符
    calculate_jantou_fu(analyzer, status, &mut details)?;

    // 待ちの符
    calculate_machi_fu(analyzer, hand, &mut details)?;

    // ツモ符
    calculate_tsumo_fu(analyzer, status, &mut details)?;

    // 門前ロン加符
    calculate_menzen_ron_fu(status, &mut details)?;

    let raw_total: u32 = details.iter().map(|d| d.fu).sum();

    // 平和ツモは20符固定
    if is_pinfu(analyzer, hand, status) && status.is_self_picked {
        return Ok(FuResult {
            total: 20,
            details: vec![FuDetail {
                name: "平和ツモ",
                fu: 20,
            }],
        });
    }

    // 鳴き平和形（副底のみ）のロンは30符
    let total = if raw_total == 20 && !status.is_self_picked && status.has_claimed_open {
        30
    } else {
        // 10符単位に切り上げ
        round_up_to_10(raw_total)
    };

    Ok(FuResult { total, details })
}

/// 10符単位に切り上げる
fn round_up_to_10(fu: u32) -> u32 {
    (fu + 9) / 10 * 10
}

/// 平和判定（簡易版：符計算用）
fn is_pinfu(analyzer: &HandAnalyzer, hand: &Hand, status: &Status) -> bool {
    if status.has_claimed_open {
        return false;
    }
    if analyzer.form != Form::Normal {
        return false;
    }
    if analyzer.sequential3.len() != 4 || analyzer.same2.len() != 1 {
        return false;
    }
    // 雀頭が役牌でないこと
    for head in &analyzer.same2 {
        let tile = head.get()[0];
        if is_yakuhai_tile(tile, status) {
            return false;
        }
    }
    // 両面待ちであること
    if let Some(winning_tile) = hand.drawn() {
        for seq in &analyzer.sequential3 {
            let tiles = seq.get();
            if winning_tile.get() == tiles[0] && tiles[0] % 9 != 6 {
                return true;
            }
            if winning_tile.get() == tiles[2] && tiles[2] % 9 != 2 {
                return true;
            }
        }
        return false;
    }
    false
}

/// 役牌かどうかを判定する
fn is_yakuhai_tile(tile: TileType, status: &Status) -> bool {
    // 三元牌
    if Dragon::is_tile_type(tile).is_some() {
        return true;
    }
    // 自風牌
    if Wind::is_tile_type(tile) == Some(status.player_wind) {
        return true;
    }
    // 場風牌
    if Wind::is_tile_type(tile) == Some(status.prevailing_wind) {
        return true;
    }
    false
}

/// 面子（刻子・槓子・順子）の符を計算する
fn calculate_mentsu_fu(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    // 副露面子の牌種を収集（analyzer.same3 との重複排除用）
    let opened_triplet_tiles: Vec<TileType> = hand
        .opened()
        .iter()
        .filter(|o| o.category == OpenType::Pon || o.category == OpenType::Kan)
        .map(|o| o.tiles[0].get())
        .collect();

    // 門前手の刻子（暗刻）
    for same in &analyzer.same3 {
        let tile = same.get()[0];

        // 副露面子として既にカウントされる刻子はスキップ
        if opened_triplet_tiles.contains(&tile) {
            continue;
        }

        let is_terminal_or_honor = is_terminal_or_honor(tile);

        // 和了牌を含む刻子がロン和了の場合は明刻扱い
        let is_concealed = if !status.is_self_picked {
            if let Some(drawn) = hand.drawn() {
                drawn.get() != tile
            } else {
                true
            }
        } else {
            true
        };

        let fu = if is_concealed {
            if is_terminal_or_honor { 8 } else { 4 }
        } else {
            if is_terminal_or_honor { 4 } else { 2 }
        };

        let name = if is_concealed {
            if is_terminal_or_honor {
                "么九牌暗刻"
            } else {
                "中張牌暗刻"
            }
        } else {
            if is_terminal_or_honor {
                "么九牌明刻"
            } else {
                "中張牌明刻"
            }
        };

        details.push(FuDetail { name, fu });
    }

    // 副露面子
    for open in hand.opened() {
        match open.category {
            OpenType::Pon => {
                let tile = open.tiles[0].get();
                let is_terminal_or_honor = is_terminal_or_honor(tile);
                let fu = if is_terminal_or_honor { 4 } else { 2 };
                let name = if is_terminal_or_honor {
                    "么九牌明刻"
                } else {
                    "中張牌明刻"
                };
                details.push(FuDetail { name, fu });
            }
            OpenType::Kan => {
                let tile = open.tiles[0].get();
                let is_terminal_or_honor = is_terminal_or_honor(tile);
                let is_concealed = open.from == OpenFrom::Myself;
                let fu = if is_concealed {
                    if is_terminal_or_honor { 32 } else { 16 }
                } else {
                    if is_terminal_or_honor { 16 } else { 8 }
                };
                let name = if is_concealed {
                    if is_terminal_or_honor {
                        "么九牌暗槓"
                    } else {
                        "中張牌暗槓"
                    }
                } else {
                    if is_terminal_or_honor {
                        "么九牌明槓"
                    } else {
                        "中張牌明槓"
                    }
                };
                details.push(FuDetail { name, fu });
            }
            OpenType::Chi => {
                // チーの順子は0符
            }
        }
    }

    Ok(())
}

/// 雀頭の符を計算する
fn calculate_jantou_fu(
    analyzer: &HandAnalyzer,
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    for head in &analyzer.same2 {
        let tile = head.get()[0];

        // 三元牌の雀頭：2符
        if Dragon::is_tile_type(tile).is_some() {
            details.push(FuDetail {
                name: "三元牌雀頭",
                fu: 2,
            });
        }

        // 自風牌の雀頭：2符
        if Wind::is_tile_type(tile) == Some(status.player_wind) {
            details.push(FuDetail {
                name: "自風牌雀頭",
                fu: 2,
            });
        }

        // 場風牌の雀頭：2符
        if Wind::is_tile_type(tile) == Some(status.prevailing_wind) {
            details.push(FuDetail {
                name: "場風牌雀頭",
                fu: 2,
            });
        }
    }

    Ok(())
}

/// 待ちの形による符を計算する
fn calculate_machi_fu(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    if let Some(winning_tile) = hand.drawn() {
        let wt = winning_tile.get();

        // 単騎待ち: 雀頭で待っていた場合
        for head in &analyzer.same2 {
            if head.get()[0] == wt {
                details.push(FuDetail {
                    name: "単騎待ち",
                    fu: 2,
                });
                return Ok(());
            }
        }

        // 嵌張待ち・辺張待ち
        for seq in &analyzer.sequential3 {
            let tiles = seq.get();
            // 嵌張待ち: 真ん中の牌で待っていた
            if wt == tiles[1] {
                details.push(FuDetail {
                    name: "嵌張待ち",
                    fu: 2,
                });
                return Ok(());
            }
            // 辺張待ち: 12の3待ち or 89の7待ち
            if wt == tiles[2] && tiles[2] % 9 == 2 {
                details.push(FuDetail {
                    name: "辺張待ち",
                    fu: 2,
                });
                return Ok(());
            }
            if wt == tiles[0] && tiles[0] % 9 == 6 {
                details.push(FuDetail {
                    name: "辺張待ち",
                    fu: 2,
                });
                return Ok(());
            }
        }

        // 両面待ちや双碰待ちは0符
    }

    Ok(())
}

/// ツモの符を計算する
fn calculate_tsumo_fu(
    _analyzer: &HandAnalyzer,
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    // ツモ和了は2符（ただし平和ツモの場合は別途処理するため、ここでは常に加算）
    if status.is_self_picked {
        details.push(FuDetail {
            name: "自摸",
            fu: 2,
        });
    }

    Ok(())
}

/// 門前ロンの加符を計算する
fn calculate_menzen_ron_fu(
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    // 門前でロン和了した場合は10符加算
    if !status.has_claimed_open && !status.is_self_picked {
        details.push(FuDetail {
            name: "門前加符",
            fu: 10,
        });
    }

    Ok(())
}

/// 么九牌（1,9）または字牌かを判定する
fn is_terminal_or_honor(tile: TileType) -> bool {
    matches!(
        tile,
        Tile::M1
            | Tile::M9
            | Tile::P1
            | Tile::P9
            | Tile::S1
            | Tile::S9
            | Tile::Z1
            | Tile::Z2
            | Tile::Z3
            | Tile::Z4
            | Tile::Z5
            | Tile::Z6
            | Tile::Z7
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;
    use crate::hand_info::hand_analyzer::HandAnalyzer;
    use crate::hand_info::status::Status;
    use crate::tile::Wind;

    /// 平和ツモは20符
    #[test]
    fn test_pinfu_tsumo() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        assert_eq!(result.total, 20);
    }

    /// 平和ロンは30符
    #[test]
    fn test_pinfu_ron() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 = 30
        assert_eq!(result.total, 30);
    }

    /// 七対子は25符固定
    #[test]
    fn test_seven_pairs() {
        let hand = Hand::from("1122m3344p5566s7z 7z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let status = Status::new();
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        assert_eq!(result.total, 25);
    }

    /// 中張牌暗刻のみの手（順子+暗刻+雀頭、ツモ）
    #[test]
    fn test_concealed_triplet_simple() {
        // 222m 123p 789s 456s 33m: ツモ和了
        // 手牌: 222m123p456789s3m ツモ3m
        let hand = Hand::from("222m123p456789s3m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 中張牌暗刻4(222m) + 単騎待ち2(3m) + ツモ2 = 28 -> 30
        assert_eq!(result.total, 30);
    }

    /// 么九牌暗刻を含む手（ロン和了）
    #[test]
    fn test_concealed_triplet_terminal() {
        // 111m 456p 789s 234m 55m: ロン5m（単騎）
        let hand = Hand::from("111m456p789s2345m 5m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 么九牌暗刻8(111m) + 単騎待ち2 = 40
        assert_eq!(result.total, 40);
    }

    /// ポンした明刻（中張牌）: 2符
    #[test]
    fn test_open_triplet_simple() {
        // 123p 789s 456s 33m + ポン222m + ツモ3m
        let hand = Hand::from("123p456789s3m 222m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_picked = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 中張牌明刻2(222m) + 単騎待ち2(3m) + ツモ2 = 26 -> 30
        assert_eq!(result.total, 30);
    }

    /// ポンした明刻（么九牌）: 4符
    #[test]
    fn test_open_triplet_terminal() {
        // 123p 456s 789s 33m + ポン111m + ツモ3m
        let hand = Hand::from("123p456789s3m 111m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_picked = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 么九牌明刻4(111m) + 単騎待ち2(3m) + ツモ2 = 28 -> 30
        assert_eq!(result.total, 30);
    }

    /// 明槓（中張牌）: 8符
    #[test]
    fn test_open_kan_simple() {
        // 123p 789s 456s 33m + 明槓2222m + ツモ3m
        let hand = Hand::from("123p456789s3m 2222m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_picked = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // from=Unknownなので明槓扱い
        // 副底20 + 中張牌明槓8 + 単騎待ち2(3m) + ツモ2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

    /// 三元牌の雀頭: 2符
    #[test]
    fn test_dragon_pair() {
        let hand = Hand::from("123456m234p789s5z 5z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 三元牌雀頭2 + 単騎待ち2 = 34 -> 40
        assert_eq!(result.total, 40);
    }

    /// 自風牌の雀頭: 2符
    #[test]
    fn test_player_wind_pair() {
        let hand = Hand::from("123456m234p789s1z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::East;
        status.prevailing_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 自風牌雀頭2 + 単騎待ち2 = 34 -> 40
        assert_eq!(result.total, 40);
    }

    /// 場風牌の雀頭: 2符
    #[test]
    fn test_prevailing_wind_pair() {
        let hand = Hand::from("123456m234p789s1z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 場風牌雀頭2 + 単騎待ち2 = 34 -> 40
        assert_eq!(result.total, 40);
    }

    /// 連風牌の雀頭（自風=場風=東）: 4符
    #[test]
    fn test_double_wind_pair() {
        let hand = Hand::from("123456m234p789s1z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::East;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 自風牌雀頭2 + 場風牌雀頭2 + 単騎待ち2 = 36 -> 40
        assert_eq!(result.total, 40);
    }

    /// 嵌張待ち: 2符
    #[test]
    fn test_kanchan_wait() {
        let hand = Hand::from("123456m234p79s11z 8s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 嵌張待ち2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

    /// 辺張待ち（12の3待ち）: 2符
    #[test]
    fn test_penchan_wait_low() {
        let hand = Hand::from("12m456m234p789s1z 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 辺張待ち2 + 場風牌雀頭2(南=2z? いや1z=東) = 32 -> 40
        // 1z=東、場風南なので雀頭加符なし
        // 副底20 + 門前加符10 + 辺張待ち2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

    /// 辺張待ち（89の7待ち）: 2符
    #[test]
    fn test_penchan_wait_high() {
        let hand = Hand::from("123m456m234p89s1z 7s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // 副底20 + 門前加符10 + 辺張待ち2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

    /// 10符単位の切り上げ
    #[test]
    fn test_round_up_to_10() {
        assert_eq!(round_up_to_10(20), 20);
        assert_eq!(round_up_to_10(21), 30);
        assert_eq!(round_up_to_10(25), 30);
        assert_eq!(round_up_to_10(29), 30);
        assert_eq!(round_up_to_10(30), 30);
        assert_eq!(round_up_to_10(31), 40);
        assert_eq!(round_up_to_10(32), 40);
    }

    /// 鳴き平和形のロンは30符
    #[test]
    fn test_open_pinfu_ron() {
        let hand = Hand::from("456m789s33z 123p 234s 3z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        assert_eq!(result.total, 30);
    }
}
