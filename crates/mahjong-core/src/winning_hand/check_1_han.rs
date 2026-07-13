use anyhow::Result;

use crate::hand::Hand;
use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::tile::Dragon;
use crate::winning_hand::name::*;

/// Riichi (立直)
pub fn check_riichi(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(Kind::Riichi, status.has_claimed_open, settings.display_lang);
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // Double Riichi replaces plain Riichi; the two never combine.
    if status.is_double_riichi {
        return Ok((name, false, 0));
    }
    if status.has_claimed_riichi {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}

/// Fully Concealed Hand (Menzen Tsumo / 門前清自摸和)
pub fn check_fully_concealed_hand(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::FullyConcealedHand,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if !status.has_claimed_open && status.is_self_drawn {
        return Ok((name, true, 1));
    }
    Ok((name, false, 0))
}

/// Unbroken (Ippatsu / 一発)
pub fn check_unbroken(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::Unbroken,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if !check_riichi(hand_analyzer, status, settings)?.1 {
        return Ok((name, false, 0));
    }
    if status.is_unbroken {
        return Ok((name, true, 1));
    }
    Ok((name, false, 0))
}
/// Last Tile Draw (Haitei / 海底撈月)
pub fn check_last_tile_draw(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LastTileDraw,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.is_last_tile_draw && status.is_self_drawn {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// Last Tile Claim (Hōtei / 河底撈魚)
pub fn check_last_tile_claim(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LastTileClaim,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.is_last_tile_claim && !status.is_self_drawn {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// After a Quad (Rinshan Kaihō / 嶺上開花)
pub fn check_after_a_quad(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AfterAQuad,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.is_after_a_quad && status.is_self_drawn {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// Robbing a Quad (Chankan / 搶槓)
pub fn check_robbing_a_quad(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::RobbingAQuad,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.is_robbing_a_quad && !status.is_self_drawn {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// Double Riichi (ダブル立直)
pub fn check_double_riichi(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::DoubleRiichi,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    if status.is_double_riichi && status.has_claimed_riichi {
        Ok((name, true, 2))
    } else {
        Ok((name, false, 0))
    }
}
/// Pinfu (平和)
pub fn check_pinfu(
    hand_analyzer: &HandAnalyzer,
    raw_hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(Kind::Pinfu, status.has_claimed_open, settings.display_lang);
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Closed hands only.
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    if hand_analyzer.sequential3.len() != 4 || hand_analyzer.same2.len() != 1 {
        return Ok((name, false, 0));
    }
    // The pair must not be a value honour (yakuhai).
    for head in &hand_analyzer.same2 {
        if head.has_dragon(Dragon::White)?
            || head.has_dragon(Dragon::Green)?
            || head.has_dragon(Dragon::Red)?
        {
            return Ok((name, false, 0));
        }
        if head.has_wind(status.seat_wind)? {
            return Ok((name, false, 0));
        }
        if head.has_wind(status.round_wind)? {
            return Ok((name, false, 0));
        }
    }
    // Pinfu requires a two-sided wait; edge, closed, and pair waits do not qualify.
    if let Some(winning_tile) = raw_hand.drawn() {
        let has_open_wait = hand_analyzer
            .sequential3
            .iter()
            .any(|seq| seq.is_two_sided_wait(winning_tile.get()));
        if !has_open_wait {
            return Ok((name, false, 0));
        }
    }
    Ok((name, true, 1))
}
/// Twin Sequences (Iipeikō / 一盃口)
pub fn check_twin_sequences(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::TwinSequences,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Closed hands only.
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    if hand_analyzer.sequential3.len() < 2 {
        return Ok((name, false, 0));
    }
    // Count identical sequence pairs to tell this apart from
    // Double Twin Sequences (二盃口).
    let mut used = vec![false; hand_analyzer.sequential3.len()];
    let mut pair_count = 0;
    for i in 0..hand_analyzer.sequential3.len() {
        if used[i] {
            continue;
        }
        for j in i + 1..hand_analyzer.sequential3.len() {
            if used[j] {
                continue;
            }
            if hand_analyzer.sequential3[i] == hand_analyzer.sequential3[j] {
                used[i] = true;
                used[j] = true;
                pair_count += 1;
                break;
            }
        }
    }
    // Two pairs would be Double Twin Sequences, which never combines.
    if pair_count == 1 {
        return Ok((name, true, 1));
    }
    Ok((name, false, 0))
}
/// All Inside (Tan'yao / 断么九)
pub fn check_all_inside(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllInside,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Under the no-kuitan rule an open hand cannot score All Inside.
    if !settings.opened_all_inside && status.has_claimed_open {
        return Ok((name, false, 0));
    }
    let mut has_1_9_honour = false;
    for same in &hand_analyzer.same3 {
        if same.has_1_or_9()? || same.has_honour()? {
            has_1_9_honour = true;
        }
    }
    for seq in &hand_analyzer.sequential3 {
        if seq.has_1_or_9()? {
            has_1_9_honour = true;
        }
    }

    for head in &hand_analyzer.same2 {
        if head.has_1_or_9()? || head.has_honour()? {
            has_1_9_honour = true;
        }
    }

    if has_1_9_honour {
        return Ok((name, false, 0));
    }

    Ok((name, true, 1))
}
/// Value Honour: seat wind (yakuhai / 役牌（自風牌）)
pub fn check_value_honour_seat_wind(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ValueHonourSeatWind,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    let mut has_player_wind = false;
    for same in &hand_analyzer.same3 {
        if same.has_wind(status.seat_wind)? {
            has_player_wind = true;
        }
    }

    if has_player_wind {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// Value Honour: round wind (yakuhai / 役牌（場風牌）)
pub fn check_value_honour_round_wind(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ValueHonourRoundWind,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    let mut has_prevailing_wind = false;
    for same in &hand_analyzer.same3 {
        if same.has_wind(status.round_wind)? {
            has_prevailing_wind = true;
        }
    }

    if has_prevailing_wind {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}

/// Whether the hand contains a triplet of the given dragon.
pub fn check_value_honour_dragons(hand_analyzer: &HandAnalyzer, dragon: Dragon) -> Result<bool> {
    if !hand_analyzer.shanten.has_won() {
        return Ok(false);
    }
    let mut has_dragon = false;
    for same in &hand_analyzer.same3 {
        if same.has_dragon(dragon)? {
            has_dragon = true;
        }
    }

    if has_dragon { Ok(true) } else { Ok(false) }
}

/// Value Honour: White dragon (yakuhai / 役牌（白）)
pub fn check_value_honour_white_dragon(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ValueHonourWhiteDragon,
        status.has_claimed_open,
        settings.display_lang,
    );
    if check_value_honour_dragons(hand_analyzer, Dragon::White)? {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// Value Honour: Green dragon (yakuhai / 役牌（發）)
pub fn check_value_honour_green_dragon(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ValueHonourGreenDragon,
        status.has_claimed_open,
        settings.display_lang,
    );
    if check_value_honour_dragons(hand_analyzer, Dragon::Green)? {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// Value Honour: Red dragon (yakuhai / 役牌（中）)
pub fn check_value_honour_red_dragon(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ValueHonourRedDragon,
        status.has_claimed_open,
        settings.display_lang,
    );
    if check_value_honour_dragons(hand_analyzer, Dragon::Red)? {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hand::*, tile::*};
    #[test]
    fn test_win_by_riichi() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_riichi = true;
        assert_eq!(
            check_riichi(&test_analyzer, &status, &settings).unwrap(),
            ("立直", true, 1)
        );
    }
    #[test]
    fn test_win_by_unbroken() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_riichi = true;
        status.is_unbroken = true;
        assert_eq!(
            check_unbroken(&test_analyzer, &status, &settings).unwrap(),
            ("一発", true, 1)
        );
    }
    #[test]
    fn test_win_by_fully_concealed_hand() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_self_drawn = true;
        assert_eq!(
            check_fully_concealed_hand(&test_analyzer, &status, &settings).unwrap(),
            ("門前清自摸和", true, 1)
        );
    }
    #[test]
    fn test_not_win_by_fully_concealed_hand_with_claiming_open() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_self_drawn = true;
        status.has_claimed_open = true;
        assert_eq!(
            check_fully_concealed_hand(&test_analyzer, &status, &settings).unwrap(),
            ("門前清自摸和", false, 0)
        );
    }
    #[test]
    fn test_win_by_all_inside_open_rule_close_hand() {
        let test_str = "222456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    fn test_not_win_by_all_inside_with_1() {
        let test_str = "111456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    fn test_not_win_by_all_inside_with_9() {
        let test_str = "222456m777p5699s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    fn test_not_win_by_all_inside_with_honour() {
        let test_str = "222456m56s88s111z 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    fn test_win_by_all_inside_open_rule_open_hand() {
        let test_str = "234m567m234p345s3s 3s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = true;
        status.has_claimed_open = true;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    fn test_win_by_all_inside_close_rule_close_hand() {
        let test_str = "678m23455p33345ss 5p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = false;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    fn test_win_by_all_inside_close_rule_open_hand() {
        let test_str = "222m456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        rules.opened_all_inside = false;
        status.has_claimed_open = true;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    fn test_win_by_twin_sequences() {
        let test_str = "112233m456p456s7z 7z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_twin_sequences(&test_analyzer, &status, &settings).unwrap(),
            ("一盃口", true, 1)
        );
    }
    #[test]
    fn test_no_win_by_twin_sequences_with_opened() {
        let test_str = "112233m456p456s7z 7z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_twin_sequences(&test_analyzer, &status, &settings).unwrap(),
            ("一盃口", false, 0)
        );
    }
    #[test]
    fn test_win_by_pinfu() {
        let test_str = "123567m234p6799s 5s";
        let test = Hand::from(test_str);
        let analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_pinfu(&analyzer, &test, &status, &settings).unwrap(),
            ("平和", true, 1)
        );
    }
    #[test]
    fn test_not_win_by_pinfu_with_open() {
        let test_str = "123567m6799s 234p 5s";
        let test = Hand::from(test_str);
        let analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_pinfu(&analyzer, &test, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }
    #[test]
    fn test_not_win_by_pinfu_with_triplet() {
        let test_str = "123456m789p222s3s 3s";
        let test = Hand::from(test_str);
        let analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_pinfu(&analyzer, &test, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }
    #[test]
    fn test_not_win_by_pinfu_with_edge_wait() {
        let test_str = "12567m234p56799s 3m";
        let test = Hand::from(test_str);
        let analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_pinfu(&analyzer, &test, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }

    #[test]
    fn test_not_win_by_pinfu_with_closed_wait() {
        let test_str = "123567m234p5799s 6s";
        let test = Hand::from(test_str);
        let analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_pinfu(&analyzer, &test, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }
    #[test]
    fn test_not_win_by_pinfu_with_honour_pair() {
        let test_str = "123567m234p67s11z 8s";
        let test = Hand::from(test_str);
        let analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.seat_wind = Wind::East;
        status.round_wind = Wind::East;
        assert_eq!(
            check_pinfu(&analyzer, &test, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }
    #[test]
    fn test_win_by_value_honour_seat_wind() {
        let test_str = "222m456m777p5s 222z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.round_wind = Wind::East;
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_seat_wind(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（自風牌）", true, 1)
        );
    }
    #[test]
    fn test_win_by_value_honour_round_wind() {
        let test_str = "222m456m777p5s 111z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.round_wind = Wind::East;
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_round_wind(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（場風牌）", true, 1)
        );
    }
    #[test]
    fn test_win_by_value_honour_white_dragon() {
        let test_str = "222m456m777p5s 555z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.round_wind = Wind::East;
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_white_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（白）", true, 1)
        );
    }
    #[test]
    fn test_win_by_value_honour_green_dragon() {
        let test_str = "222m456m777p5s 666z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.round_wind = Wind::East;
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_green_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（發）", true, 1)
        );
    }
    #[test]
    fn test_win_by_value_honour_red_dragon() {
        let test_str = "222m456m777p5s 777z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.round_wind = Wind::East;
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_red_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（中）", true, 1)
        );
    }
    #[test]
    fn test_win_by_last_tile_draw() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_last_tile_draw = true;
        status.is_self_drawn = true;
        assert_eq!(
            check_last_tile_draw(&test_analyzer, &status, &settings).unwrap(),
            ("海底撈月", true, 1)
        );
    }
    #[test]
    fn test_not_win_by_last_tile_draw_without_self_drawn() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_last_tile_draw = true;
        status.is_self_drawn = false;
        assert_eq!(
            check_last_tile_draw(&test_analyzer, &status, &settings).unwrap(),
            ("海底撈月", false, 0)
        );
    }
    #[test]
    fn test_win_by_last_tile_claim() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_last_tile_claim = true;
        status.is_self_drawn = false;
        assert_eq!(
            check_last_tile_claim(&test_analyzer, &status, &settings).unwrap(),
            ("河底撈魚", true, 1)
        );
    }
    #[test]
    fn test_win_by_after_a_quad() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_after_a_quad = true;
        status.is_self_drawn = true;
        assert_eq!(
            check_after_a_quad(&test_analyzer, &status, &settings).unwrap(),
            ("嶺上開花", true, 1)
        );
    }
    #[test]
    fn test_win_by_robbing_a_quad() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_robbing_a_quad = true;
        status.is_self_drawn = false;
        assert_eq!(
            check_robbing_a_quad(&test_analyzer, &status, &settings).unwrap(),
            ("搶槓", true, 1)
        );
    }
    #[test]
    fn test_win_by_double_riichi() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_riichi = true;
        status.is_double_riichi = true;
        assert_eq!(
            check_double_riichi(&test_analyzer, &status, &settings).unwrap(),
            ("ダブル立直", true, 2)
        );
    }
    #[test]
    fn test_not_win_by_double_riichi_without_ready() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_riichi = false;
        status.is_double_riichi = true;
        assert_eq!(
            check_double_riichi(&test_analyzer, &status, &settings).unwrap(),
            ("ダブル立直", false, 0)
        );
    }
}
