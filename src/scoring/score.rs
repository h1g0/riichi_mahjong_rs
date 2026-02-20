use std::collections::HashMap;

use anyhow::Result;

use crate::hand::Hand;
use crate::hand_info::hand_analyzer::HandAnalyzer;
use crate::hand_info::status::Status;
use crate::scoring::fu::{calculate_fu, FuResult};
use crate::winning_hand::checker;
use crate::winning_hand::name::Kind;
use crate::settings::Settings;

/// 点数計算の結果
#[derive(Debug, PartialEq, Eq)]
pub struct ScoreResult {
    /// 翻数
    pub han: u32,
    /// 符
    pub fu: u32,
    /// 点数等級名称
    pub rank: ScoreRank,
    /// 親の場合のロン和了点
    pub dealer_ron: u32,
    /// 親の場合のツモ和了点（各子の支払い）
    pub dealer_tsumo_all: u32,
    /// 子の場合のロン和了点
    pub non_dealer_ron: u32,
    /// 子の場合のツモ和了点（親の支払い）
    pub non_dealer_tsumo_dealer: u32,
    /// 子の場合のツモ和了点（子の支払い）
    pub non_dealer_tsumo_non_dealer: u32,
    /// 成立した役の一覧
    pub yaku_list: Vec<(&'static str, u32)>,
    /// 符の内訳
    pub fu_result: FuResult,
}

/// 点数の等級
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScoreRank {
    /// 通常（満貫未満）
    Normal,
    /// 満貫（5翻以上、または役満以外で3翻60符以上・4翻30符以上）
    Mangan,
    /// 跳満（6～7翻）
    Haneman,
    /// 倍満（8～10翻）
    Baiman,
    /// 三倍満（11～12翻）
    Sanbaiman,
    /// 役満（13翻以上）
    Yakuman,
}

/// 点数を計算する
///
/// # Arguments
/// * `analyzer` - 手牌解析結果
/// * `hand` - 手牌
/// * `status` - 局の状態
/// * `settings` - ルール設定
///
/// # Returns
/// 点数計算の結果。役がない場合はNone。
pub fn calculate_score(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<Option<ScoreResult>> {
    // 役判定
    let yaku_result = checker::check(analyzer, hand, status, settings)?;

    // 成立した役を抽出
    let yaku_list = extract_yaku_list(&yaku_result);

    if yaku_list.is_empty() {
        return Ok(None);
    }

    // 翻数の合計
    let han: u32 = yaku_list.iter().map(|(_, h)| h).sum();

    // 役満判定
    let has_yakuman = yaku_list.iter().any(|(_, h)| *h >= 13);

    // 符計算
    let fu_result = calculate_fu(analyzer, hand, status)?;
    let fu = fu_result.total;

    // 等級を決定
    let rank = determine_rank(han, fu, has_yakuman);

    // 基本点を計算
    let base_points = calculate_base_points(han, fu, rank);

    // 各支払い額を計算
    let dealer_ron = round_up_to_100(base_points * 6);
    let dealer_tsumo_all = round_up_to_100(base_points * 2);
    let non_dealer_ron = round_up_to_100(base_points * 4);
    let non_dealer_tsumo_dealer = round_up_to_100(base_points * 2);
    let non_dealer_tsumo_non_dealer = round_up_to_100(base_points);

    Ok(Some(ScoreResult {
        han,
        fu,
        rank,
        dealer_ron,
        dealer_tsumo_all,
        non_dealer_ron,
        non_dealer_tsumo_dealer,
        non_dealer_tsumo_non_dealer,
        yaku_list,
        fu_result,
    }))
}

/// 役判定結果から成立した役のリストを抽出する
fn extract_yaku_list(
    yaku_result: &HashMap<Kind, (&'static str, bool, u32)>,
) -> Vec<(&'static str, u32)> {
    let mut list: Vec<(&'static str, u32)> = Vec::new();
    let mut has_yakuman = false;

    // まず役満があるか確認
    for (_, (_, is_valid, han)) in yaku_result {
        if *is_valid && *han >= 13 {
            has_yakuman = true;
            break;
        }
    }

    for (_, (name, is_valid, han)) in yaku_result {
        if *is_valid && *han > 0 {
            // 役満がある場合は通常役を除外
            if has_yakuman && *han < 13 {
                continue;
            }
            list.push((name, *han));
        }
    }

    // 翻数の降順でソートし、同じ翻数の場合は名前でソート
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list
}

/// 等級を決定する
fn determine_rank(han: u32, fu: u32, has_yakuman: bool) -> ScoreRank {
    if has_yakuman || han >= 13 {
        ScoreRank::Yakuman
    } else if han >= 11 {
        ScoreRank::Sanbaiman
    } else if han >= 8 {
        ScoreRank::Baiman
    } else if han >= 6 {
        ScoreRank::Haneman
    } else if han >= 5 {
        ScoreRank::Mangan
    } else if han == 4 && fu >= 30 {
        ScoreRank::Mangan
    } else if han == 3 && fu >= 60 {
        ScoreRank::Mangan
    } else {
        ScoreRank::Normal
    }
}

/// 基本点を計算する
fn calculate_base_points(han: u32, fu: u32, rank: ScoreRank) -> u32 {
    match rank {
        ScoreRank::Yakuman => 8000,
        ScoreRank::Sanbaiman => 6000,
        ScoreRank::Baiman => 4000,
        ScoreRank::Haneman => 3000,
        ScoreRank::Mangan => 2000,
        ScoreRank::Normal => {
            // 基本点 = 符 × 2^(翻+2)
            let base = fu * (1 << (han + 2));
            // 満貫を超えないようにする
            if base > 2000 { 2000 } else { base }
        }
    }
}

/// 100点単位に切り上げる
fn round_up_to_100(points: u32) -> u32 {
    (points + 99) / 100 * 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;
    use crate::hand_info::hand_analyzer::HandAnalyzer;
    use crate::hand_info::status::Status;
    use crate::settings::Settings;
    use crate::tile::Wind;

    /// 満貫の子ロン: 8000点
    #[test]
    fn test_mangan_non_dealer_ron() {
        let rank = ScoreRank::Mangan;
        let base = calculate_base_points(5, 30, rank);
        assert_eq!(base, 2000);
        assert_eq!(round_up_to_100(base * 4), 8000);
    }

    /// 満貫の親ロン: 12000点
    #[test]
    fn test_mangan_dealer_ron() {
        let rank = ScoreRank::Mangan;
        let base = calculate_base_points(5, 30, rank);
        assert_eq!(base, 2000);
        assert_eq!(round_up_to_100(base * 6), 12000);
    }

    /// 跳満の子ロン: 12000点
    #[test]
    fn test_haneman_non_dealer_ron() {
        let rank = ScoreRank::Haneman;
        let base = calculate_base_points(6, 30, rank);
        assert_eq!(base, 3000);
        assert_eq!(round_up_to_100(base * 4), 12000);
    }

    /// 倍満の子ロン: 16000点
    #[test]
    fn test_baiman_non_dealer_ron() {
        let rank = ScoreRank::Baiman;
        let base = calculate_base_points(8, 30, rank);
        assert_eq!(base, 4000);
        assert_eq!(round_up_to_100(base * 4), 16000);
    }

    /// 三倍満の子ロン: 24000点
    #[test]
    fn test_sanbaiman_non_dealer_ron() {
        let rank = ScoreRank::Sanbaiman;
        let base = calculate_base_points(11, 30, rank);
        assert_eq!(base, 6000);
        assert_eq!(round_up_to_100(base * 4), 24000);
    }

    /// 役満の子ロン: 32000点
    #[test]
    fn test_yakuman_non_dealer_ron() {
        let rank = ScoreRank::Yakuman;
        let base = calculate_base_points(13, 30, rank);
        assert_eq!(base, 8000);
        assert_eq!(round_up_to_100(base * 4), 32000);
    }

    /// 1翻30符の子ロン: 1000点
    #[test]
    fn test_1han_30fu_non_dealer_ron() {
        let rank = determine_rank(1, 30, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(1, 30, rank);
        // 30 * 2^3 = 240
        assert_eq!(base, 240);
        // 240 * 4 = 960 -> 切り上げ 1000
        assert_eq!(round_up_to_100(base * 4), 1000);
    }

    /// 1翻40符の子ロン: 1300点
    #[test]
    fn test_1han_40fu_non_dealer_ron() {
        let rank = determine_rank(1, 40, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(1, 40, rank);
        // 40 * 2^3 = 320
        assert_eq!(base, 320);
        // 320 * 4 = 1280 -> 切り上げ 1300
        assert_eq!(round_up_to_100(base * 4), 1300);
    }

    /// 2翻30符の子ロン: 2000点
    #[test]
    fn test_2han_30fu_non_dealer_ron() {
        let rank = determine_rank(2, 30, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(2, 30, rank);
        // 30 * 2^4 = 480
        assert_eq!(base, 480);
        // 480 * 4 = 1920 -> 切り上げ 2000
        assert_eq!(round_up_to_100(base * 4), 2000);
    }

    /// 3翻30符の子ロン: 4000点
    #[test]
    fn test_3han_30fu_non_dealer_ron() {
        let rank = determine_rank(3, 30, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(3, 30, rank);
        // 30 * 2^5 = 960
        assert_eq!(base, 960);
        // 960 * 4 = 3840 -> 切り上げ 3900
        assert_eq!(round_up_to_100(base * 4), 3900);
    }

    /// 3翻60符の子ロンは満貫: 8000点
    #[test]
    fn test_3han_60fu_is_mangan() {
        let rank = determine_rank(3, 60, false);
        assert_eq!(rank, ScoreRank::Mangan);
    }

    /// 4翻30符の子ロンは満貫: 8000点
    #[test]
    fn test_4han_30fu_is_mangan() {
        let rank = determine_rank(4, 30, false);
        assert_eq!(rank, ScoreRank::Mangan);
    }

    /// 4翻25符は通常計算（七対子）: 子ロン6400点
    #[test]
    fn test_4han_25fu_is_normal() {
        let rank = determine_rank(4, 25, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(4, 25, rank);
        // 25 * 2^6 = 1600
        assert_eq!(base, 1600);
        // 1600 * 4 = 6400
        assert_eq!(round_up_to_100(base * 4), 6400);
    }

    /// 100点単位の切り上げ
    #[test]
    fn test_round_up_to_100() {
        assert_eq!(round_up_to_100(100), 100);
        assert_eq!(round_up_to_100(101), 200);
        assert_eq!(round_up_to_100(960), 1000);
        assert_eq!(round_up_to_100(1920), 2000);
        assert_eq!(round_up_to_100(3840), 3900);
    }

    /// 等級の判定
    #[test]
    fn test_determine_rank() {
        assert_eq!(determine_rank(1, 30, false), ScoreRank::Normal);
        assert_eq!(determine_rank(2, 30, false), ScoreRank::Normal);
        assert_eq!(determine_rank(3, 30, false), ScoreRank::Normal);
        assert_eq!(determine_rank(3, 60, false), ScoreRank::Mangan);
        assert_eq!(determine_rank(4, 25, false), ScoreRank::Normal);
        assert_eq!(determine_rank(4, 30, false), ScoreRank::Mangan);
        assert_eq!(determine_rank(5, 30, false), ScoreRank::Mangan);
        assert_eq!(determine_rank(6, 30, false), ScoreRank::Haneman);
        assert_eq!(determine_rank(7, 30, false), ScoreRank::Haneman);
        assert_eq!(determine_rank(8, 30, false), ScoreRank::Baiman);
        assert_eq!(determine_rank(10, 30, false), ScoreRank::Baiman);
        assert_eq!(determine_rank(11, 30, false), ScoreRank::Sanbaiman);
        assert_eq!(determine_rank(12, 30, false), ScoreRank::Sanbaiman);
        assert_eq!(determine_rank(13, 30, false), ScoreRank::Yakuman);
        assert_eq!(determine_rank(13, 30, true), ScoreRank::Yakuman);
    }

    /// 満貫の子ツモ: 親4000 + 子2000×2 = 8000
    #[test]
    fn test_mangan_non_dealer_tsumo() {
        let base = calculate_base_points(5, 30, ScoreRank::Mangan);
        let dealer_pay = round_up_to_100(base * 2);  // 4000
        let non_dealer_pay = round_up_to_100(base);   // 2000
        assert_eq!(dealer_pay, 4000);
        assert_eq!(non_dealer_pay, 2000);
    }

    /// 満貫の親ツモ: 子4000×3 = 12000
    #[test]
    fn test_mangan_dealer_tsumo() {
        let base = calculate_base_points(5, 30, ScoreRank::Mangan);
        let each_pay = round_up_to_100(base * 2);  // 4000
        assert_eq!(each_pay, 4000);
    }

    /// 立直のみ（門前ロン）: 1翻30符 -> 子ロン1000点
    #[test]
    fn test_calculate_score_riichi_only() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_ready = true;
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        // 平和 + 立直 = 2翻, 30符
        assert_eq!(result.han, 2);
        assert_eq!(result.fu, 30);
        assert_eq!(result.non_dealer_ron, 2000);
    }

    /// ツモで和了（門前清自摸和 + 平和）: 2翻20符 -> 子ツモ: 親700 + 子400×2
    #[test]
    fn test_calculate_score_tsumo_pinfu() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        // 門前清自摸和 + 平和 = 2翻, 20符
        assert_eq!(result.han, 2);
        assert_eq!(result.fu, 20);
        // 基本点 = 20 * 2^4 = 320
        // 子ツモ: 親700(640->700) + 子400(320->400)×2
        assert_eq!(result.non_dealer_tsumo_dealer, 700);
        assert_eq!(result.non_dealer_tsumo_non_dealer, 400);
    }

    /// 役がない手は None を返す
    #[test]
    fn test_calculate_score_no_yaku() {
        let hand = Hand::from("123456m234p789s3z 3z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.has_claimed_open = true;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings).unwrap();
        assert!(result.is_none());
    }

    /// 役満（国士無双）: 子ロン32000点
    #[test]
    fn test_calculate_score_yakuman() {
        let hand = Hand::from("19m19p19s1234567z 1m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        status.player_wind = Wind::South;
        status.prevailing_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        assert_eq!(result.rank, ScoreRank::Yakuman);
        assert_eq!(result.non_dealer_ron, 32000);
        assert_eq!(result.dealer_ron, 48000);
    }

    /// 2翻40符の親ロン: 2600点
    #[test]
    fn test_2han_40fu_dealer_ron() {
        let rank = determine_rank(2, 40, false);
        let base = calculate_base_points(2, 40, rank);
        // 40 * 2^4 = 640
        assert_eq!(base, 640);
        // 640 * 6 = 3840 -> 切り上げ 3900
        assert_eq!(round_up_to_100(base * 6), 3900);
    }

    /// 1翻30符の親ツモ: 各子500点
    #[test]
    fn test_1han_30fu_dealer_tsumo() {
        let base = calculate_base_points(1, 30, ScoreRank::Normal);
        // 30 * 2^3 = 240
        assert_eq!(base, 240);
        // 240 * 2 = 480 -> 切り上げ 500
        assert_eq!(round_up_to_100(base * 2), 500);
    }

    /// 1翻30符の子ツモ: 親500点, 子300点
    #[test]
    fn test_1han_30fu_non_dealer_tsumo() {
        let base = calculate_base_points(1, 30, ScoreRank::Normal);
        // 親: 240 * 2 = 480 -> 500
        assert_eq!(round_up_to_100(base * 2), 500);
        // 子: 240 * 1 = 240 -> 300
        assert_eq!(round_up_to_100(base), 300);
    }
}
