use anyhow::Result;

use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::tile::Dragon;
use crate::winning_hand::name::*;

/// 立直
pub fn check_ready_hand(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ReadyHand,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    if status.has_claimed_ready {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}

/// 門前清自摸和
pub fn check_self_pick(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::SelfPick,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if !status.has_claimed_open && status.is_self_picked {
        return Ok((name, true, 1));
    }
    Ok((name, false, 0))
}

/// 一発
pub fn check_one_shot(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::OneShot,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if !check_ready_hand(hand, status, settings)?.1 {
        return Ok((name, false, 0));
    }
    if status.is_one_shot {
        return Ok((name, true, 1));
    }
    Ok((name, false, 0))
}
/// 海底撈月
pub fn check_last_tile_from_the_wall(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LastTileFromTheWall,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if status.is_last_tile_from_the_wall && status.is_self_picked {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// 河底撈魚
pub fn check_last_discard(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LastDiscard,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if status.is_last_discard && !status.is_self_picked {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// 嶺上開花
pub fn check_dead_wall_draw(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::DeadWallDraw,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if status.is_dead_wall_draw && status.is_self_picked {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// 搶槓
pub fn check_robbing_a_quad(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::RobbingAQuad,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if status.is_robbing_a_quad && !status.is_self_picked {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// ダブル立直
pub fn check_double_ready(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::DoubleReady,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    if status.is_double_ready && status.has_claimed_ready {
        Ok((name, true, 2))
    } else {
        Ok((name, false, 0))
    }
}
/// 平和
pub fn check_no_points_hand(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::NoPointsHand,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 門前でなければ平和は成立しない
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // 4つの順子と1つの雀頭で構成されている必要がある
    if hand.sequential3.len() != 4 || hand.same2.len() != 1 {
        return Ok((name, false, 0));
    }
    // 雀頭が役牌でないこと
    for head in &hand.same2 {
        // 三元牌は不可
        if head.has_dragon(Dragon::White)?
            || head.has_dragon(Dragon::Green)?
            || head.has_dragon(Dragon::Red)?
        {
            return Ok((name, false, 0));
        }
        // 自風牌は不可
        if head.has_wind(status.player_wind)? {
            return Ok((name, false, 0));
        }
        // 場風牌は不可
        if head.has_wind(status.prevailing_wind)? {
            return Ok((name, false, 0));
        }
    }
    Ok((name, true, 1))
}
/// 一盃口
pub fn check_one_set_of_identical_sequences(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::OneSetOfIdenticalSequences,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 鳴いていたら一盃口は成立しない
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // 順子が2つ以上なければ一盃口はありえない
    if hand.sequential3.len() < 2 {
        return Ok((name, false, 0));
    }
    for i in 0..hand.sequential3.len() - 1 {
        if let Some(v) = hand.sequential3.get(i) {
            for j in i + 1..hand.sequential3.len() {
                if let Some(v2) = hand.sequential3.get(j) {
                    if *v == *v2 {
                        return Ok((name, true, 1));
                    }
                }
            }
        }
    }
    Ok((name, false, 0))
}
/// 断么九
pub fn check_all_simples(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllSimples,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 喰いタンなしなら鳴いている時点で抜ける
    if !settings.opened_all_simples && status.has_claimed_open {
        return Ok((name, false, 0));
    }
    let mut has_1_9_honor = false;
    // 面子

    // 刻子
    for same in &hand.same3 {
        if same.has_1_or_9()? || same.has_honor()? {
            has_1_9_honor = true;
        }
    }
    // 順子
    for seq in &hand.sequential3 {
        if seq.has_1_or_9()? {
            has_1_9_honor = true;
        }
    }

    // 雀頭
    for head in &hand.same2 {
        if head.has_1_or_9()? || head.has_honor()? {
            has_1_9_honor = true;
        }
    }

    if has_1_9_honor {
        return Ok((name, false, 0));
    }

    Ok((name, true, 1))
}
/// 役牌（自風牌）
pub fn check_honor_tiles_players_wind(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HonorTilesPlayersWind,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    let mut has_player_wind = false;
    // 刻子
    for same in &hand.same3 {
        if same.has_wind(status.player_wind)? {
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
pub fn check_honor_tiles_prevailing_wind(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HonorTilesPrevailingWind,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    let mut has_prevailing_wind = false;
    // 刻子
    for same in &hand.same3 {
        if same.has_wind(status.prevailing_wind)? {
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
pub fn check_honor_tiles_dragons(hand: &HandAnalyzer, dragon: Dragon) -> Result<bool> {
    if !has_won(hand) {
        return Ok(false);
    }
    let mut has_dragon = false;
    // 刻子
    for same in &hand.same3 {
        if same.has_dragon(dragon)? {
            has_dragon = true;
        }
    }

    if has_dragon {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// 役牌（白）
pub fn check_honor_tiles_white_dragon(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HonorTilesWhiteDragon,
        status.has_claimed_open,
        settings.display_lang,
    );
    if check_honor_tiles_dragons(hand, Dragon::White)? {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// 役牌（發）
pub fn check_honor_tiles_green_dragon(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HonorTilesGreenDragon,
        status.has_claimed_open,
        settings.display_lang,
    );
    if check_honor_tiles_dragons(hand, Dragon::Green)? {
        Ok((name, true, 1))
    } else {
        Ok((name, false, 0))
    }
}
/// 役牌（中）
pub fn check_honor_tiles_red_dragon(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HonorTilesRedDragon,
        status.has_claimed_open,
        settings.display_lang,
    );
    if check_honor_tiles_dragons(hand, Dragon::Red)? {
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
    fn test_win_by_ready_hand() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_ready = true;
        assert_eq!(
            check_ready_hand(&test_analyzer, &status, &settings).unwrap(),
            ("立直", true, 1)
        );
    }
    #[test]
    /// 立直に一発が付いた
    fn test_win_by_one_shot() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_ready = true;
        status.is_one_shot = true;
        assert_eq!(
            check_one_shot(&test_analyzer, &status, &settings).unwrap(),
            ("一発", true, 1)
        );
    }
    #[test]
    /// 門前清自摸和で和了った
    fn test_win_by_self_pick() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_self_picked = true;
        assert_eq!(
            check_self_pick(&test_analyzer, &status, &settings).unwrap(),
            ("門前清自摸和", true, 1)
        );
    }
    #[test]
    /// 鳴いている場合は門前清自摸和は付かない
    fn test_not_win_by_self_pick_with_claiming_open() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_self_picked = true;
        status.has_claimed_open = true;
        assert_eq!(
            check_self_pick(&test_analyzer, &status, &settings).unwrap(),
            ("門前清自摸和", false, 0)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断あり鳴きなし）
    fn test_win_by_all_simples_open_rule_close_hand() {
        let test_str = "222456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_simples = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    /// 么九牌ありでは断么九にならない（一）
    fn test_not_win_by_all_simples_with_1() {
        let test_str = "111456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_simples = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 么九牌ありでは断么九にならない（九）
    fn test_not_win_by_all_simples_with_9() {
        let test_str = "222456m777p5699s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_simples = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 么九牌ありでは断么九にならない（字牌）
    fn test_not_win_by_all_simples_with_honor() {
        let test_str = "222456m56s88s111z 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きなし
        rules.opened_all_simples = true;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断あり鳴きあり）
    fn test_win_by_all_simples_open_rule_open_hand() {
        let test_str = "234m567m234p345s3s 3s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断あり鳴きあり
        rules.opened_all_simples = true;
        status.has_claimed_open = true;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断なし鳴きなし）
    fn test_win_by_all_simples_close_rule_close_hand() {
        let test_str = "678m23455p33345ss 5p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断なし鳴きなし
        rules.opened_all_simples = false;
        status.has_claimed_open = false;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", true, 1)
        );
    }
    #[test]
    /// 断么九で和了った（喰い断なし鳴きあり）->役無し
    fn test_win_by_all_simples_close_rule_open_hand() {
        let test_str = "222m456m777p56s88s 7s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let mut rules = Settings::new();
        // 喰い断なし鳴きあり（役無し）
        rules.opened_all_simples = false;
        status.has_claimed_open = true;
        assert_eq!(
            check_all_simples(&test_analyzer, &status, &rules).unwrap(),
            ("断么九", false, 0)
        );
    }
    #[test]
    /// 一盃口で和了った
    fn test_win_by_one_set_of_identical_sequences() {
        let test_str = "112233m456p456s7z 7z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_one_set_of_identical_sequences(&test_analyzer, &status, &settings).unwrap(),
            ("一盃口", true, 1)
        );
    }
    #[test]
    /// 一盃口で和了った（鳴きあり）→役なし
    fn test_no_win_by_one_set_of_identical_sequences_with_opened() {
        let test_str = "112233m456p456s7z 7z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_one_set_of_identical_sequences(&test_analyzer, &status, &settings).unwrap(),
            ("一盃口", false, 0)
        );
    }
    #[test]
    /// 自風で和了った
    fn test_win_by_honor_tiles_players_wind() {
        let test_str = "222m456m777p5s 222z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.prevailing_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.player_wind = Wind::South;
        assert_eq!(
            check_honor_tiles_players_wind(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（自風牌）", true, 1)
        );
    }
    #[test]
    /// 場風で和了った
    fn test_win_by_honor_tiles_prevailing_wind() {
        let test_str = "222m456m777p5s 111z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.prevailing_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.player_wind = Wind::South;
        assert_eq!(
            check_honor_tiles_prevailing_wind(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（場風牌）", true, 1)
        );
    }
    #[test]
    /// 三元牌（白）で和了った
    fn test_win_by_honor_tiles_white_dragon() {
        let test_str = "222m456m777p5s 555z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.prevailing_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.player_wind = Wind::South;
        assert_eq!(
            check_honor_tiles_white_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（白）", true, 1)
        );
    }
    #[test]
    /// 三元牌（發）で和了った
    fn test_win_by_honor_tiles_green_dragon() {
        let test_str = "222m456m777p5s 666z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.prevailing_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.player_wind = Wind::South;
        assert_eq!(
            check_honor_tiles_green_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（發）", true, 1)
        );
    }
    #[test]
    /// 三元牌（中）で和了った
    fn test_win_by_honor_tiles_red_dragon() {
        let test_str = "222m456m777p5s 777z 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        // 東場
        status.prevailing_wind = Wind::East;
        // プレイヤーは南家=`2z`
        status.player_wind = Wind::South;
        assert_eq!(
            check_honor_tiles_red_dragon(&test_analyzer, &status, &settings).unwrap(),
            ("役牌（中）", true, 1)
        );
    }
    #[test]
    /// 海底撈月で和了った
    fn test_win_by_last_tile_from_the_wall() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_last_tile_from_the_wall = true;
        status.is_self_picked = true;
        assert_eq!(
            check_last_tile_from_the_wall(&test_analyzer, &status, &settings).unwrap(),
            ("海底撈月", true, 1)
        );
    }
    #[test]
    /// 海底撈月はツモ和了でなければ成立しない
    fn test_not_win_by_last_tile_from_the_wall_without_self_pick() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_last_tile_from_the_wall = true;
        status.is_self_picked = false;
        assert_eq!(
            check_last_tile_from_the_wall(&test_analyzer, &status, &settings).unwrap(),
            ("海底撈月", false, 0)
        );
    }
    #[test]
    /// 河底撈魚で和了った
    fn test_win_by_last_discard() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_last_discard = true;
        status.is_self_picked = false;
        assert_eq!(
            check_last_discard(&test_analyzer, &status, &settings).unwrap(),
            ("河底撈魚", true, 1)
        );
    }
    #[test]
    /// 嶺上開花で和了った
    fn test_win_by_dead_wall_draw() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dead_wall_draw = true;
        status.is_self_picked = true;
        assert_eq!(
            check_dead_wall_draw(&test_analyzer, &status, &settings).unwrap(),
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
        status.is_self_picked = false;
        assert_eq!(
            check_robbing_a_quad(&test_analyzer, &status, &settings).unwrap(),
            ("搶槓", true, 1)
        );
    }
    #[test]
    /// ダブル立直で和了った
    fn test_win_by_double_ready() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_ready = true;
        status.is_double_ready = true;
        assert_eq!(
            check_double_ready(&test_analyzer, &status, &settings).unwrap(),
            ("ダブル立直", true, 2)
        );
    }
    #[test]
    /// ダブル立直は立直していなければ成立しない
    fn test_not_win_by_double_ready_without_ready() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_ready = false;
        status.is_double_ready = true;
        assert_eq!(
            check_double_ready(&test_analyzer, &status, &settings).unwrap(),
            ("ダブル立直", false, 0)
        );
    }
    #[test]
    /// 平和で和了った
    fn test_win_by_no_points_hand() {
        let test_str = "123567m234p6799s 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_no_points_hand(&test_analyzer, &status, &settings).unwrap(),
            ("平和", true, 1)
        );
    }
    #[test]
    /// 平和は鳴いていたら成立しない
    fn test_not_win_by_no_points_hand_with_open() {
        let test_str = "123m234p6799s 567m 5s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_no_points_hand(&test_analyzer, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }
    #[test]
    /// 刻子があると平和にならない
    fn test_not_win_by_no_points_hand_with_triplet() {
        let test_str = "111m456m789p78s33z 9s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_no_points_hand(&test_analyzer, &status, &settings).unwrap(),
            ("平和", false, 0)
        );
    }
}
