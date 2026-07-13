use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::hand::Hand;
use crate::hand_info::hand_analyzer::HandAnalyzer;
use crate::hand_info::status::Status;
use crate::scoring::fu::{FuResult, calculate_fu};
use crate::settings::{Lang, Settings};
use crate::winning_hand::checker;
use crate::winning_hand::name::Kind;

/// Result of a score calculation.
#[derive(Debug, PartialEq, Eq)]
pub struct ScoreResult {
    /// Han
    pub han: u32,
    /// Fu (minipoints)
    pub fu: u32,
    /// Score rank (mangan and above)
    pub rank: ScoreRank,
    /// Points for a dealer win by ron
    pub dealer_ron: u32,
    /// Points for a dealer win by tsumo (paid by each non-dealer)
    pub dealer_tsumo_all: u32,
    /// Points for a non-dealer win by ron
    pub non_dealer_ron: u32,
    /// Points for a non-dealer win by tsumo (paid by the dealer)
    pub non_dealer_tsumo_dealer: u32,
    /// Points for a non-dealer win by tsumo (paid by each non-dealer)
    pub non_dealer_tsumo_non_dealer: u32,
    /// Awarded yaku and dora, as (item, han) pairs
    pub yaku_list: Vec<(ScoreItem, u32)>,
    /// Whether the hand was open; kept so the "(Open)" suffix on yaku
    /// names can be reconstructed on display
    pub has_opened: bool,
    /// Fu breakdown
    pub fu_result: FuResult,
}

/// One line of the score breakdown shown on the result screen.
///
/// Held as a typed value rather than a pre-formatted string so the
/// displaying client can localize it into any language.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum ScoreItem {
    Yaku(Kind),
    Dora(DoraLabel),
}

impl ScoreItem {
    /// Returns the display name.
    ///
    /// `has_opened` selects the "(Open)" form of yaku names that lose han
    /// when open; it is ignored for dora.
    pub fn name(&self, has_opened: bool, lang: Lang) -> &'static str {
        match self {
            ScoreItem::Yaku(kind) => crate::winning_hand::name::get(*kind, has_opened, lang),
            ScoreItem::Dora(label) => label.name(lang),
        }
    }
}

/// Score rank.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ScoreRank {
    /// Below mangan
    Normal,
    /// Mangan (満貫): 5 han, or 4 han 30+ fu / 3 han 60+ fu
    Mangan,
    /// Haneman (跳満): 6-7 han
    Haneman,
    /// Baiman (倍満): 8-10 han
    Baiman,
    /// Sanbaiman (三倍満): 11-12 han
    Sanbaiman,
    /// Yakuman (役満): a yakuman hand or 13+ han
    Yakuman,
}

impl ScoreRank {
    /// Returns the display name; empty for `Normal`.
    ///
    /// English names follow WRC Rules 2025 (see docs/glossary.md).
    pub fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => match self {
                ScoreRank::Normal => "",
                ScoreRank::Mangan => "Mangan",
                ScoreRank::Haneman => "Haneman",
                ScoreRank::Baiman => "Baiman",
                ScoreRank::Sanbaiman => "Sanbaiman",
                ScoreRank::Yakuman => "Yakuman",
            },
            Lang::Ja => match self {
                ScoreRank::Normal => "",
                ScoreRank::Mangan => "満貫",
                ScoreRank::Haneman => "跳満",
                ScoreRank::Baiman => "倍満",
                ScoreRank::Sanbaiman => "三倍満",
                ScoreRank::Yakuman => "役満",
            },
        }
    }
}

/// Kind of dora, listed next to yaku on the result screen.
///
/// Dora is not a yaku, but it contributes han and is displayed the same way.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum DoraLabel {
    Dora,
    /// Red five (赤ドラ)
    RedDora,
    /// Ura dora (裏ドラ), revealed only on a riichi win
    UraDora,
    /// Pei dora (北ドラ): in three-player games, one han per extracted North tile
    PeiDora,
}

impl DoraLabel {
    /// Returns the display name.
    ///
    /// English names follow WRC Rules 2025 (see docs/glossary.md).
    pub fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => match self {
                DoraLabel::Dora => "Dora",
                DoraLabel::RedDora => "Red Five",
                DoraLabel::UraDora => "Ura Dora",
                DoraLabel::PeiDora => "Pei Dora",
            },
            Lang::Ja => match self {
                DoraLabel::Dora => "ドラ",
                DoraLabel::RedDora => "赤ドラ",
                DoraLabel::UraDora => "裏ドラ",
                DoraLabel::PeiDora => "北ドラ",
            },
        }
    }
}

/// Calculates the score of a winning hand.
///
/// Returns `None` when the hand has no yaku.
pub fn calculate_score(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<Option<ScoreResult>> {
    let yaku_result = checker::check(analyzer, hand, status, settings)?;

    let yaku_list = extract_yaku_list(&yaku_result);

    if yaku_list.is_empty() {
        return Ok(None);
    }

    let han: u32 = yaku_list.iter().map(|(_, h)| h).sum();

    let has_yakuman = yaku_list.iter().any(|(_, h)| *h >= 13);

    let fu_result = calculate_fu(analyzer, hand, status)?;
    let fu = fu_result.total;

    let rank = determine_rank(han, fu, has_yakuman);

    let base_points = calculate_base_points(han, fu, rank);

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
        has_opened: status.has_claimed_open,
        fu_result,
    }))
}

/// Extracts the awarded yaku from the checker result.
fn extract_yaku_list(
    yaku_result: &HashMap<Kind, (&'static str, bool, u32)>,
) -> Vec<(ScoreItem, u32)> {
    let mut list: Vec<(&Kind, u32)> = Vec::new();
    let mut has_yakuman = false;

    for (_, is_valid, han) in yaku_result.values() {
        if *is_valid && *han >= 13 {
            has_yakuman = true;
            break;
        }
    }

    for (kind, (_name, is_valid, han)) in yaku_result {
        if *is_valid && *han > 0 {
            // A yakuman supersedes all ordinary yaku.
            if has_yakuman && *han < 13 {
                continue;
            }
            list.push((kind, *han));
        }
    }

    // Sort by ascending han, then by Kind declaration order,
    // which fixes the display order on the result screen.
    list.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(b.0)));
    list.into_iter()
        .map(|(kind, han)| (ScoreItem::Yaku(*kind), han))
        .collect()
}

/// Determines the score rank, including mangan rounding up
/// (4 han 30 fu / 3 han 60 fu count as mangan).
pub fn determine_rank(han: u32, fu: u32, has_yakuman: bool) -> ScoreRank {
    if has_yakuman || han >= 13 {
        ScoreRank::Yakuman
    } else if han >= 11 {
        ScoreRank::Sanbaiman
    } else if han >= 8 {
        ScoreRank::Baiman
    } else if han >= 6 {
        ScoreRank::Haneman
    } else if han >= 5 || (han == 4 && fu >= 30) || (han == 3 && fu >= 60) {
        ScoreRank::Mangan
    } else {
        ScoreRank::Normal
    }
}

/// Calculates the base points.
pub fn calculate_base_points(han: u32, fu: u32, rank: ScoreRank) -> u32 {
    match rank {
        ScoreRank::Yakuman => 8000,
        ScoreRank::Sanbaiman => 6000,
        ScoreRank::Baiman => 4000,
        ScoreRank::Haneman => 3000,
        ScoreRank::Mangan => 2000,
        ScoreRank::Normal => {
            // base points = fu x 2^(han+2)
            let base = fu * (1 << (han + 2));
            // Cap at mangan.
            if base > 2000 { 2000 } else { base }
        }
    }
}

/// Rounds points up to the next multiple of 100.
pub fn round_up_to_100(points: u32) -> u32 {
    points.div_ceil(100) * 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;
    use crate::hand_info::hand_analyzer::HandAnalyzer;
    use crate::hand_info::status::Status;
    use crate::settings::Settings;
    use crate::tile::Wind;

    /// Mangan, non-dealer ron: 8000 points.
    #[test]
    fn test_mangan_non_dealer_ron() {
        let rank = ScoreRank::Mangan;
        let base = calculate_base_points(5, 30, rank);
        assert_eq!(base, 2000);
        assert_eq!(round_up_to_100(base * 4), 8000);
    }

    /// Mangan, dealer ron: 12000 points.
    #[test]
    fn test_mangan_dealer_ron() {
        let rank = ScoreRank::Mangan;
        let base = calculate_base_points(5, 30, rank);
        assert_eq!(base, 2000);
        assert_eq!(round_up_to_100(base * 6), 12000);
    }

    /// Haneman, non-dealer ron: 12000 points.
    #[test]
    fn test_haneman_non_dealer_ron() {
        let rank = ScoreRank::Haneman;
        let base = calculate_base_points(6, 30, rank);
        assert_eq!(base, 3000);
        assert_eq!(round_up_to_100(base * 4), 12000);
    }

    /// Baiman, non-dealer ron: 16000 points.
    #[test]
    fn test_baiman_non_dealer_ron() {
        let rank = ScoreRank::Baiman;
        let base = calculate_base_points(8, 30, rank);
        assert_eq!(base, 4000);
        assert_eq!(round_up_to_100(base * 4), 16000);
    }

    /// Sanbaiman, non-dealer ron: 24000 points.
    #[test]
    fn test_sanbaiman_non_dealer_ron() {
        let rank = ScoreRank::Sanbaiman;
        let base = calculate_base_points(11, 30, rank);
        assert_eq!(base, 6000);
        assert_eq!(round_up_to_100(base * 4), 24000);
    }

    /// Yakuman, non-dealer ron: 32000 points.
    #[test]
    fn test_yakuman_non_dealer_ron() {
        let rank = ScoreRank::Yakuman;
        let base = calculate_base_points(13, 30, rank);
        assert_eq!(base, 8000);
        assert_eq!(round_up_to_100(base * 4), 32000);
    }

    /// 1 han 30 fu, non-dealer ron: 1000 points.
    #[test]
    fn test_1han_30fu_non_dealer_ron() {
        let rank = determine_rank(1, 30, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(1, 30, rank);
        assert_eq!(base, 240);
        assert_eq!(round_up_to_100(base * 4), 1000);
    }

    /// 1 han 40 fu, non-dealer ron: 1300 points.
    #[test]
    fn test_1han_40fu_non_dealer_ron() {
        let rank = determine_rank(1, 40, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(1, 40, rank);
        assert_eq!(base, 320);
        assert_eq!(round_up_to_100(base * 4), 1300);
    }

    /// 2 han 30 fu, non-dealer ron: 2000 points.
    #[test]
    fn test_2han_30fu_non_dealer_ron() {
        let rank = determine_rank(2, 30, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(2, 30, rank);
        assert_eq!(base, 480);
        assert_eq!(round_up_to_100(base * 4), 2000);
    }

    /// 3 han 30 fu, non-dealer ron: 3900 points.
    #[test]
    fn test_3han_30fu_non_dealer_ron() {
        let rank = determine_rank(3, 30, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(3, 30, rank);
        assert_eq!(base, 960);
        assert_eq!(round_up_to_100(base * 4), 3900);
    }

    /// 3 han 60 fu rounds up to mangan.
    #[test]
    fn test_3han_60fu_is_mangan() {
        let rank = determine_rank(3, 60, false);
        assert_eq!(rank, ScoreRank::Mangan);
    }

    /// 4 han 30 fu rounds up to mangan.
    #[test]
    fn test_4han_30fu_is_mangan() {
        let rank = determine_rank(4, 30, false);
        assert_eq!(rank, ScoreRank::Mangan);
    }

    /// 4 han 25 fu (Seven Pairs) stays below mangan: non-dealer ron 6400.
    #[test]
    fn test_4han_25fu_is_normal() {
        let rank = determine_rank(4, 25, false);
        assert_eq!(rank, ScoreRank::Normal);
        let base = calculate_base_points(4, 25, rank);
        assert_eq!(base, 1600);
        assert_eq!(round_up_to_100(base * 4), 6400);
    }

    #[test]
    fn test_round_up_to_100() {
        assert_eq!(round_up_to_100(100), 100);
        assert_eq!(round_up_to_100(101), 200);
        assert_eq!(round_up_to_100(960), 1000);
        assert_eq!(round_up_to_100(1920), 2000);
        assert_eq!(round_up_to_100(3840), 3900);
    }

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

    /// Mangan, non-dealer tsumo: dealer 4000 + 2000 from each non-dealer.
    #[test]
    fn test_mangan_non_dealer_tsumo() {
        let base = calculate_base_points(5, 30, ScoreRank::Mangan);
        let dealer_pay = round_up_to_100(base * 2); // 4000
        let non_dealer_pay = round_up_to_100(base); // 2000
        assert_eq!(dealer_pay, 4000);
        assert_eq!(non_dealer_pay, 2000);
    }

    /// Mangan, dealer tsumo: 4000 from every non-dealer.
    #[test]
    fn test_mangan_dealer_tsumo() {
        let base = calculate_base_points(5, 30, ScoreRank::Mangan);
        let each_pay = round_up_to_100(base * 2); // 4000
        assert_eq!(each_pay, 4000);
    }

    #[test]
    fn test_calculate_score_riichi_only() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_riichi = true;
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        // Pinfu + Riichi = 2 han, 30 fu
        assert_eq!(result.han, 2);
        assert_eq!(result.fu, 30);
        assert_eq!(result.non_dealer_ron, 2000);
    }

    #[test]
    fn test_calculate_score_tsumo_pinfu() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        // Fully Concealed Hand + Pinfu = 2 han, 20 fu
        assert_eq!(result.han, 2);
        assert_eq!(result.fu, 20);
        // base 20 * 2^4 = 320: dealer pays 640 -> 700, others 320 -> 400
        assert_eq!(result.non_dealer_tsumo_dealer, 700);
        assert_eq!(result.non_dealer_tsumo_non_dealer, 400);
    }

    #[test]
    fn test_calculate_score_no_yaku() {
        let hand = Hand::from("123456m234p789s3z 3z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.has_claimed_open = true;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_score_yakuman() {
        let hand = Hand::from("19m19p19s1234567z 1m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        assert_eq!(result.rank, ScoreRank::Yakuman);
        assert_eq!(result.non_dealer_ron, 32000);
        assert_eq!(result.dealer_ron, 48000);
    }

    /// 2 han 40 fu, dealer ron: 3900 points.
    #[test]
    fn test_2han_40fu_dealer_ron() {
        let rank = determine_rank(2, 40, false);
        let base = calculate_base_points(2, 40, rank);
        assert_eq!(base, 640);
        assert_eq!(round_up_to_100(base * 6), 3900);
    }

    /// 1 han 30 fu, dealer tsumo: 500 from each non-dealer.
    #[test]
    fn test_1han_30fu_dealer_tsumo() {
        let base = calculate_base_points(1, 30, ScoreRank::Normal);
        assert_eq!(base, 240);
        assert_eq!(round_up_to_100(base * 2), 500);
    }

    /// 1 han 30 fu, non-dealer tsumo: dealer 500, non-dealers 300.
    #[test]
    fn test_1han_30fu_non_dealer_tsumo() {
        let base = calculate_base_points(1, 30, ScoreRank::Normal);
        assert_eq!(round_up_to_100(base * 2), 500);
        assert_eq!(round_up_to_100(base), 300);
    }

    /// The yaku list is sorted by ascending han:
    /// All Inside (1 han) before Seven Pairs (2 han).
    #[test]
    fn test_yaku_list_order_han_ascending() {
        let hand = Hand::from("2244668m224466p 8m");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        assert_eq!(result.yaku_list[0], (ScoreItem::Yaku(Kind::AllInside), 1));
        assert_eq!(result.yaku_list[1], (ScoreItem::Yaku(Kind::SevenPairs), 2));
    }

    /// Equal-han yaku are sorted by Kind declaration order:
    /// Riichi before Pinfu.
    #[test]
    fn test_yaku_list_order_same_han_uses_kind_order() {
        let hand = Hand::from("123456m234p6799s 5s");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.has_claimed_riichi = true;
        status.is_self_drawn = false;
        status.seat_wind = Wind::South;
        status.round_wind = Wind::East;
        let settings = Settings::new();
        let result = calculate_score(&analyzer, &hand, &status, &settings)
            .unwrap()
            .unwrap();
        let items: Vec<ScoreItem> = result.yaku_list.iter().map(|(item, _)| *item).collect();
        let riichi_pos = items
            .iter()
            .position(|&i| i == ScoreItem::Yaku(Kind::Riichi))
            .unwrap();
        let pinfu_pos = items
            .iter()
            .position(|&i| i == ScoreItem::Yaku(Kind::Pinfu))
            .unwrap();
        assert!(riichi_pos < pinfu_pos);
    }

    #[test]
    fn rank_name_ja() {
        assert_eq!(ScoreRank::Normal.name(Lang::Ja), "");
        assert_eq!(ScoreRank::Mangan.name(Lang::Ja), "満貫");
        assert_eq!(ScoreRank::Haneman.name(Lang::Ja), "跳満");
        assert_eq!(ScoreRank::Baiman.name(Lang::Ja), "倍満");
        assert_eq!(ScoreRank::Sanbaiman.name(Lang::Ja), "三倍満");
        assert_eq!(ScoreRank::Yakuman.name(Lang::Ja), "役満");
    }

    #[test]
    fn rank_name_en() {
        assert_eq!(ScoreRank::Normal.name(Lang::En), "");
        assert_eq!(ScoreRank::Mangan.name(Lang::En), "Mangan");
        assert_eq!(ScoreRank::Haneman.name(Lang::En), "Haneman");
        assert_eq!(ScoreRank::Baiman.name(Lang::En), "Baiman");
        assert_eq!(ScoreRank::Sanbaiman.name(Lang::En), "Sanbaiman");
        assert_eq!(ScoreRank::Yakuman.name(Lang::En), "Yakuman");
    }

    #[test]
    fn dora_label_name_ja() {
        assert_eq!(DoraLabel::Dora.name(Lang::Ja), "ドラ");
        assert_eq!(DoraLabel::RedDora.name(Lang::Ja), "赤ドラ");
        assert_eq!(DoraLabel::UraDora.name(Lang::Ja), "裏ドラ");
        assert_eq!(DoraLabel::PeiDora.name(Lang::Ja), "北ドラ");
    }

    #[test]
    fn dora_label_name_en() {
        assert_eq!(DoraLabel::Dora.name(Lang::En), "Dora");
        assert_eq!(DoraLabel::RedDora.name(Lang::En), "Red Five");
        assert_eq!(DoraLabel::UraDora.name(Lang::En), "Ura Dora");
        assert_eq!(DoraLabel::PeiDora.name(Lang::En), "Pei Dora");
    }
}
