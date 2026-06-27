use anyhow::Result;

use crate::hand::Hand;
use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::tile::Dragon;
use crate::winning_hand::name::*;

/// 立直
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
    // ダブル立直の場合は通常の立直とは複合しない（ダブル立直が立直を置き換える）
    if status.is_double_riichi {
        return Ok((name, false, 0));
    }
    if status.has_claimed_riichi {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}

/// 門前清自摸和
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

/// 一発
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
/// 海底撈月
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
/// 河底撈魚
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
/// 嶺上開花
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
/// 搶槓
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
/// ダブル立直
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
/// 平和
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
    // 門前でなければ平和は成立しない
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // 4つの順子と1つの雀頭で構成されている必要がある
    if hand_analyzer.sequential3.len() != 4 || hand_analyzer.same2.len() != 1 {
        return Ok((name, false, 0));
    }
    // 雀頭が役牌でないこと
    for head in &hand_analyzer.same2 {
        // 三元牌は不可
        if head.has_dragon(Dragon::White)?
            || head.has_dragon(Dragon::Green)?
            || head.has_dragon(Dragon::Red)?
        {
            return Ok((name, false, 0));
        }
        // 自風牌は不可
        if head.has_wind(status.seat_wind)? {
            return Ok((name, false, 0));
        }
        // 場風牌は不可
        if head.has_wind(status.round_wind)? {
            return Ok((name, false, 0));
        }
    }
    // 平和は両面待ちのみ成立（辺張・嵌張・単騎は不可）
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
/// 一盃口
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
    // 鳴いていたら一盃口は成立しない
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // 順子が2つ以上なければ一盃口はありえない
    if hand_analyzer.sequential3.len() < 2 {
        return Ok((name, false, 0));
    }
    // 同一順子ペアの数をカウント（二盃口との区別のため）
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
    // 二盃口（ペアが2組）の場合は一盃口とは複合しない
    if pair_count == 1 {
        return Ok((name, true, 1));
    }
    Ok((name, false, 0))
}
/// 断么九
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
    // 喰いタンなしなら鳴いている時点で抜ける
    if !settings.opened_all_inside && status.has_claimed_open {
        return Ok((name, false, 0));
    }
    let mut has_1_9_honour = false;
    // 面子

    // 刻子
    for same in &hand_analyzer.same3 {
        if same.has_1_or_9()? || same.has_honour()? {
            has_1_9_honour = true;
        }
    }
    // 順子
    for seq in &hand_analyzer.sequential3 {
        if seq.has_1_or_9()? {
            has_1_9_honour = true;
        }
    }

    // 雀頭
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
/// 役牌（自風牌）
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
    // 刻子
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
/// 役牌（場風牌）
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
    // 刻子
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

/// 面子に三元牌の順子が含まれるか調べる
pub fn check_value_honour_dragons(hand_analyzer: &HandAnalyzer, dragon: Dragon) -> Result<bool> {
    if !hand_analyzer.shanten.has_won() {
        return Ok(false);
    }
    let mut has_dragon = false;
    // 刻子
    for same in &hand_analyzer.same3 {
        if same.has_dragon(dragon)? {
            has_dragon = true;
        }
    }

    if has_dragon { Ok(true) } else { Ok(false) }
}

/// 役牌（白）
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
/// 役牌（發）
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
/// 役牌（中）
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

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hand::*, tile::*};
    #[test]
    /// 立直で和了った
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
    /// 立直に一発が付いた
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
    /// 門前清自摸和で和了った
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
    /// 鳴いている場合は門前清自摸和は付かない
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
    /// 断么九で和了った（喰い断あり鳴きなし）
    fn test_win_by_all_inside_open_rule_close_hand() {
        let test_str = "222456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    /// 么九牌ありでは断么九にならない（一）
    fn test_not_win_by_all_inside_with_1() {
        let test_str = "111456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 么九牌ありでは断么九にならない（九）
    fn test_not_win_by_all_inside_with_9() {
        let test_str = "222456m777p5699s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 么九牌ありでは断么九にならない（字牌）
    fn test_not_win_by_all_inside_with_honour() {
        let test_str = "222456m56s88s111z 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_inside = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断あり鳴きあり）
    fn test_win_by_all_inside_open_rule_open_hand() {
        let test_str = "234m567m234p345s3s 3s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きあり
        rules.opened_all_inside = true;
        status.has_claimed_open = true;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断なし鳴きなし）
    fn test_win_by_all_inside_close_rule_close_hand() {
        let test_str = "678m23455p33345ss 5p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断なし鳴きなし
        rules.opened_all_inside = false;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断なし鳴きあり）->役無し
    fn test_win_by_all_inside_close_rule_open_hand() {
        let test_str = "222m456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断なし鳴きあり（役無し）
        rules.opened_all_inside = false;
        status.has_claimed_open = true;
        assert_eq!(
            check_all_inside(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 一盃口で和了った
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
    /// 一盃口で和了った（鳴きあり）→役なし
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
    /// 平和で和了った
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
    /// 鳴いていると平和にならない
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
    /// 刻子が含まれると平和にならない
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
    /// 両面待ちでないと平和にならない（辺張待ち）
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
    /// 両面待ちでないと平和にならない（嵌張待ち）
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
    /// 雀頭が役牌だと平和にならない
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
    /// 自風で和了った
    fn test_win_by_value_honour_seat_wind() {
        let test_str = "222m456m777p5s 222z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.round_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_seat_wind(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（自風牌）", true, 1)
        );
    }
    #[test]
    /// 場風で和了った
    fn test_win_by_value_honour_round_wind() {
        let test_str = "222m456m777p5s 111z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.round_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_round_wind(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（場風牌）", true, 1)
        );
    }
    #[test]
    /// 三元牌（白）で和了った
    fn test_win_by_value_honour_white_dragon() {
        let test_str = "222m456m777p5s 555z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.round_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_white_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（白）", true, 1)
        );
    }
    #[test]
    /// 三元牌（發）で和了った
    fn test_win_by_value_honour_green_dragon() {
        let test_str = "222m456m777p5s 666z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.round_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_green_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（發）", true, 1)
        );
    }
    #[test]
    /// 三元牌（中）で和了った
    fn test_win_by_value_honour_red_dragon() {
        let test_str = "222m456m777p5s 777z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.round_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.seat_wind = Wind::South;
        assert_eq!(
            check_value_honour_red_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（中）", true, 1)
        );
    }
    #[test]
    /// 海底撈月で和了った
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
    /// 海底撈月はツモ和了でなければ成立しない
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
    /// 河底撈魚で和了った
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
    /// 嶺上開花で和了った
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
    /// 搶槓で和了った
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
    /// ダブル立直で和了った
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
    /// ダブル立直は立直していなければ成立しない
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
