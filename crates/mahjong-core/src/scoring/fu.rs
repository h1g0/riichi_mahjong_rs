use anyhow::Result;

use crate::hand::Hand;
use crate::hand_info::hand_analyzer::HandAnalyzer;
use crate::hand_info::meld::{MeldFrom, MeldType};
use crate::hand_info::status::Status;
use crate::tile::{Dragon, Tile, TileType, Wind, suit_rank};
use crate::winning_hand::name::Form;

/// Result of a minipoints (fu / 符) calculation.
#[derive(Debug, PartialEq, Eq)]
pub struct FuResult {
    /// Total fu, already rounded up to the next 10
    pub total: u32,
    /// Itemized breakdown
    pub details: Vec<FuDetail>,
}

/// One item of the fu breakdown.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FuDetail {
    /// Display name of the item
    pub name: &'static str,
    /// Fu awarded
    pub fu: u32,
}

/// Calculates minipoints (fu).
///
/// Returns the rounded-up total together with the itemized breakdown.
pub fn calculate_fu(analyzer: &HandAnalyzer, hand: &Hand, status: &Status) -> Result<FuResult> {
    // Seven Pairs is always exactly 25 fu.
    if analyzer.form == Form::SevenPairs {
        return Ok(FuResult {
            total: 25,
            details: vec![FuDetail {
                name: "七対子",
                fu: 25,
            }],
        });
    }

    // Thirteen Orphans has no fu calculation; 30 is used by convention.
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

    // Base fu (futei / 副底).
    details.push(FuDetail {
        name: "副底",
        fu: 20,
    });

    calculate_mentsu_fu(analyzer, hand, status, &mut details)?;

    calculate_jantou_fu(analyzer, status, &mut details)?;

    calculate_machi_fu(analyzer, hand, &mut details)?;

    calculate_tsumo_fu(analyzer, status, &mut details)?;

    calculate_menzen_ron_fu(status, &mut details)?;

    let raw_total: u32 = details.iter().map(|d| d.fu).sum();

    // Pinfu + tsumo is fixed at 20 fu (the tsumo 2 fu does not apply).
    if is_pinfu(analyzer, hand, status) && status.is_self_drawn {
        return Ok(FuResult {
            total: 20,
            details: vec![FuDetail {
                name: "平和ツモ",
                fu: 20,
            }],
        });
    }

    // An open pinfu-shaped hand won by ron would be a bare 20 fu;
    // it is bumped to 30 by convention.
    let total = if raw_total == 20 && !status.is_self_drawn && status.has_claimed_open {
        30
    } else {
        round_up_to_10(raw_total)
    };

    Ok(FuResult { total, details })
}

/// Rounds fu up to the next multiple of 10.
fn round_up_to_10(fu: u32) -> u32 {
    fu.div_ceil(10) * 10
}

/// Simplified pinfu test, used only to decide the fixed-fu cases above.
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
    // The pair must not be a value honour (yakuhai).
    for head in &analyzer.same2 {
        let tile = head.get()[0];
        if is_yakuhai_tile(tile, status) {
            return false;
        }
    }
    // Pinfu requires a two-sided wait.
    if let Some(winning_tile) = hand.drawn() {
        for seq in &analyzer.sequential3 {
            if seq.is_two_sided_wait(winning_tile.get()) {
                return true;
            }
        }
        return false;
    }
    false
}

/// Whether the tile is a value honour (yakuhai / 役牌).
fn is_yakuhai_tile(tile: TileType, status: &Status) -> bool {
    if Dragon::is_tile_type(tile).is_some() {
        return true;
    }
    if Wind::is_tile_type(tile) == Some(status.seat_wind) {
        return true;
    }
    if Wind::is_tile_type(tile) == Some(status.round_wind) {
        return true;
    }
    false
}

/// Fu from groups (triplets, quads, sequences).
fn calculate_mentsu_fu(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    // Tile kinds of melded triplets/quads: analyzer.same3 also contains
    // melded groups, so these must not be counted twice.
    let opened_triplet_tiles: Vec<TileType> = hand
        .melds()
        .iter()
        .filter(|o| o.category == MeldType::Pon || o.category.is_kan())
        .map(|o| o.tiles[0].get())
        .collect();

    // Concealed triplets.
    for same in &analyzer.same3 {
        let tile = same.get()[0];

        if opened_triplet_tiles.contains(&tile) {
            continue;
        }

        let is_terminal_or_honour = Tile::new(tile).is_1_9_honour();

        // A triplet completed by a ron tile counts as open.
        let is_concealed = if !status.is_self_drawn {
            if let Some(drawn) = hand.drawn() {
                drawn.get() != tile
            } else {
                true
            }
        } else {
            true
        };

        let fu = if is_concealed {
            if is_terminal_or_honour { 8 } else { 4 }
        } else {
            if is_terminal_or_honour { 4 } else { 2 }
        };

        let name = if is_concealed {
            if is_terminal_or_honour {
                "么九牌暗刻"
            } else {
                "中張牌暗刻"
            }
        } else {
            if is_terminal_or_honour {
                "么九牌明刻"
            } else {
                "中張牌明刻"
            }
        };

        details.push(FuDetail { name, fu });
    }

    for open in hand.melds() {
        match open.category {
            MeldType::Pon => {
                let is_terminal_or_honour = open.tiles[0].is_1_9_honour();
                let fu = if is_terminal_or_honour { 4 } else { 2 };
                let name = if is_terminal_or_honour {
                    "么九牌明刻"
                } else {
                    "中張牌明刻"
                };
                details.push(FuDetail { name, fu });
            }
            MeldType::Kan | MeldType::Kakan => {
                let is_terminal_or_honour = open.tiles[0].is_1_9_honour();
                let is_concealed = open.from == MeldFrom::Myself;
                let fu = if is_concealed {
                    if is_terminal_or_honour { 32 } else { 16 }
                } else {
                    if is_terminal_or_honour { 16 } else { 8 }
                };
                let name = if is_concealed {
                    if is_terminal_or_honour {
                        "么九牌暗槓"
                    } else {
                        "中張牌暗槓"
                    }
                } else {
                    if is_terminal_or_honour {
                        "么九牌明槓"
                    } else {
                        "中張牌明槓"
                    }
                };
                details.push(FuDetail { name, fu });
            }
            MeldType::Chi => {
                // Sequences score no fu.
            }
        }
    }

    Ok(())
}

/// Fu from the pair.
fn calculate_jantou_fu(
    analyzer: &HandAnalyzer,
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    for head in &analyzer.same2 {
        let tile = head.get()[0];

        if Dragon::is_tile_type(tile).is_some() {
            details.push(FuDetail {
                name: "三元牌雀頭",
                fu: 2,
            });
        }

        if Wind::is_tile_type(tile) == Some(status.seat_wind) {
            details.push(FuDetail {
                name: "自風牌雀頭",
                fu: 2,
            });
        }

        if Wind::is_tile_type(tile) == Some(status.round_wind) {
            details.push(FuDetail {
                name: "場風牌雀頭",
                fu: 2,
            });
        }
    }

    Ok(())
}

/// Fu from the wait shape.
fn calculate_machi_fu(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    if let Some(winning_tile) = hand.drawn() {
        let wt = winning_tile.get();

        // Pair wait (tanki / 単騎).
        for head in &analyzer.same2 {
            if head.get()[0] == wt {
                details.push(FuDetail {
                    name: "単騎待ち",
                    fu: 2,
                });
                return Ok(());
            }
        }

        for seq in &analyzer.sequential3 {
            let tiles = seq.get();
            // Closed wait (kanchan / 嵌張): won on the middle tile.
            if wt == tiles[1] {
                details.push(FuDetail {
                    name: "嵌張待ち",
                    fu: 2,
                });
                return Ok(());
            }
            // Edge wait (penchan / 辺張): 3 completing 1-2, or 7 completing 8-9.
            if wt == tiles[2] && suit_rank(tiles[2]) == Some(3) {
                details.push(FuDetail {
                    name: "辺張待ち",
                    fu: 2,
                });
                return Ok(());
            }
            if wt == tiles[0] && suit_rank(tiles[0]) == Some(7) {
                details.push(FuDetail {
                    name: "辺張待ち",
                    fu: 2,
                });
                return Ok(());
            }
        }

        // Two-sided and dual-pair waits score no fu.
    }

    Ok(())
}

/// Fu from winning by self-draw.
fn calculate_tsumo_fu(
    _analyzer: &HandAnalyzer,
    status: &Status,
    details: &mut Vec<FuDetail>,
) -> Result<()> {
    // Always added here; the pinfu + tsumo case discards the whole
    // breakdown afterwards, so no exception is needed.
    if status.is_self_drawn {
        details.push(FuDetail {
            name: "自摸",
            fu: 2,
        });
    }

    Ok(())
}

/// The 10-fu bonus for winning by ron with a closed hand.
fn calculate_menzen_ron_fu(status: &Status, details: &mut Vec<FuDetail>) -> Result<()> {
    if !status.has_claimed_open && !status.is_self_drawn {
        details.push(FuDetail {
            name: "門前加符",
            fu: 10,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;
    use crate::hand_info::hand_analyzer::HandAnalyzer;
    use crate::hand_info::status::Status;
    use crate::tile::Wind;

    #[test]
    fn test_pinfu_tsumo() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        assert_eq!(result.total, 20);
    }

    #[test]
    fn test_pinfu_ron() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 = 30
        assert_eq!(result.total, 30);
    }

    #[test]
    fn test_seven_pairs() {
        let hand = Hand::from("1122m3344p5566s7z 7z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let status = Status::new();
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        assert_eq!(result.total, 25);
    }

    #[test]
    fn test_concealed_triplet_simple() {
        let hand = Hand::from("222m123p456789s3m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + concealed inside triplet 4 (222m) + pair wait 2 + tsumo 2 = 28 -> 30
        assert_eq!(result.total, 30);
    }

    #[test]
    fn test_concealed_triplet_terminal() {
        let hand = Hand::from("111m456p789s2345m 5m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + concealed terminal triplet 8 (111m) + pair wait 2 = 40
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_open_triplet_simple() {
        let hand = Hand::from("123p456789s3m 222m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_drawn = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + open inside triplet 2 (222m) + pair wait 2 + tsumo 2 = 26 -> 30
        assert_eq!(result.total, 30);
    }

    #[test]
    fn test_open_triplet_terminal() {
        let hand = Hand::from("123p456789s3m 111m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_drawn = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + open terminal triplet 4 (111m) + pair wait 2 + tsumo 2 = 28 -> 30
        assert_eq!(result.total, 30);
    }

    #[test]
    fn test_open_kan_simple() {
        let hand = Hand::from("123p456789s3m 2222m 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_drawn = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // from=Unknown counts as a called quad:
        // base 20 + open inside quad 8 + pair wait 2 + tsumo 2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_dragon_pair() {
        let hand = Hand::from("123456m234p789s5z 5z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + dragon pair 2 + pair wait 2 = 34 -> 40
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_player_wind_pair() {
        let hand = Hand::from("123456m234p789s1z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::East;
        status.round_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + seat wind pair 2 + pair wait 2 = 34 -> 40
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_prevailing_wind_pair() {
        let hand = Hand::from("123456m234p789s1z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + round wind pair 2 + pair wait 2 = 34 -> 40
        assert_eq!(result.total, 40);
    }

    /// A double-wind pair (seat wind == round wind) scores 2 + 2 fu.
    #[test]
    fn test_double_wind_pair() {
        let hand = Hand::from("123456m234p789s1z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::East;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + seat wind pair 2 + round wind pair 2 + pair wait 2 = 36 -> 40
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_kanchan_wait() {
        let hand = Hand::from("123456m234p79s11z 8s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + closed wait 2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_penchan_wait_low() {
        let hand = Hand::from("12m456m234p789s1z 3m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + edge wait 2 = 32 -> 40
        // (the 1z pair is East, not the South round wind, so no pair fu)
        assert_eq!(result.total, 40);
    }

    #[test]
    fn test_penchan_wait_high() {
        let hand = Hand::from("123m456m234p89s1z 7s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::South;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        // base 20 + closed ron 10 + edge wait 2 = 32 -> 40
        assert_eq!(result.total, 40);
    }

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

    /// An open pinfu-shaped ron must be bumped from 20 to 30 fu.
    #[test]
    fn test_open_pinfu_ron() {
        let hand = Hand::from("456m789s33z 123p 234s 3z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_open = true;
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let result = calculate_fu(&analyzer, &hand, &status).unwrap();
        assert_eq!(result.total, 30);
    }
}
