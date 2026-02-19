use anyhow::Result;

use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::tile::{Dragon, Tile, Wind};
use crate::winning_hand::name::*;

/// 国士無双
pub fn check_thirteen_orphans(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ThirteenOrphans,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    return if hand.form == Form::ThirteenOrphans {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    };
}
/// 四暗刻
pub fn check_four_concealed_triplets(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::FourConcealedTriplets,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    if !status.has_claimed_open && hand.same3.len() == 4 && status.is_self_picked {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// 大三元
pub fn check_big_three_dragons(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::BigThreeDragons,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 大三元: 三元牌（白・發・中）の3つすべてが刻子
    let mut dragon_count = 0;
    for same in &hand.same3 {
        if same.has_dragon(Dragon::White)?
            || same.has_dragon(Dragon::Green)?
            || same.has_dragon(Dragon::Red)?
        {
            dragon_count += 1;
        }
    }
    if dragon_count == 3 {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// 小四喜
pub fn check_little_four_winds(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LittleFourWinds,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 小四喜: 風牌のうち3つが刻子、1つが雀頭
    let mut wind_triplet_count = 0;
    let mut wind_pair = false;
    for same in &hand.same3 {
        if same.has_wind(Wind::East)?
            || same.has_wind(Wind::South)?
            || same.has_wind(Wind::West)?
            || same.has_wind(Wind::North)?
        {
            wind_triplet_count += 1;
        }
    }
    for head in &hand.same2 {
        if head.has_wind(Wind::East)?
            || head.has_wind(Wind::South)?
            || head.has_wind(Wind::West)?
            || head.has_wind(Wind::North)?
        {
            wind_pair = true;
        }
    }
    if wind_triplet_count == 3 && wind_pair {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// 大四喜
pub fn check_big_four_winds(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::BigFourWinds,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 大四喜: 風牌4つすべてが刻子
    let mut wind_triplet_count = 0;
    for same in &hand.same3 {
        if same.has_wind(Wind::East)?
            || same.has_wind(Wind::South)?
            || same.has_wind(Wind::West)?
            || same.has_wind(Wind::North)?
        {
            wind_triplet_count += 1;
        }
    }
    if wind_triplet_count == 4 {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// 字一色
pub fn check_all_honors(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllHonors,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 字一色: すべての牌が字牌で構成される
    for same in &hand.same3 {
        if !same.has_honor()? {
            return Ok((name, false, 0));
        }
    }
    for head in &hand.same2 {
        if !head.has_honor()? {
            return Ok((name, false, 0));
        }
    }
    // 順子があったら字一色ではない
    if hand.sequential3.len() > 0 {
        return Ok((name, false, 0));
    }
    // 七対子形の場合もチェック（same2が7つの場合）
    if hand.form == Form::SevenPairs {
        for head in &hand.same2 {
            if !head.has_honor()? {
                return Ok((name, false, 0));
            }
        }
    }
    Ok((name, true, 13))
}
/// 清老頭
pub fn check_all_terminals(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllTerminals,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 清老頭: すべての牌が数牌の1と9のみで構成される（字牌なし・順子なし）
    if hand.sequential3.len() > 0 {
        return Ok((name, false, 0));
    }
    for same in &hand.same3 {
        if !same.has_1_or_9()? || same.has_honor()? {
            return Ok((name, false, 0));
        }
    }
    for head in &hand.same2 {
        if !head.has_1_or_9()? || head.has_honor()? {
            return Ok((name, false, 0));
        }
    }
    Ok((name, true, 13))
}
/// 緑一色
pub fn check_all_green(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllGreen,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 緑一色: 2s, 3s, 4s, 6s, 8s, 6z（發）のみで構成される
    let is_green_tile = |t: u32| -> bool {
        matches!(
            t,
            Tile::S2 | Tile::S3 | Tile::S4 | Tile::S6 | Tile::S8 | Tile::Z6
        )
    };
    for same in &hand.same3 {
        if !is_green_tile(same.get()[0]) {
            return Ok((name, false, 0));
        }
    }
    for seq in &hand.sequential3 {
        let tiles = seq.get();
        for t in &tiles {
            if !is_green_tile(*t) {
                return Ok((name, false, 0));
            }
        }
    }
    for head in &hand.same2 {
        if !is_green_tile(head.get()[0]) {
            return Ok((name, false, 0));
        }
    }
    Ok((name, true, 13))
}
/// 九蓮宝燈
pub fn check_nine_gates(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::NineGates,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 九蓮宝燈: 門前で同一種の数牌のみで、1112345678999+同種1枚の形
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // 全ブロックが同じ種類の数牌であること
    let mut has_character = false;
    let mut has_circle = false;
    let mut has_bamboo = false;
    let mut has_honor = false;

    for same in &hand.same3 {
        if same.is_character()? {
            has_character = true;
        }
        if same.is_circle()? {
            has_circle = true;
        }
        if same.is_bamboo()? {
            has_bamboo = true;
        }
        if same.has_honor()? {
            has_honor = true;
        }
    }
    for seq in &hand.sequential3 {
        if seq.is_character()? {
            has_character = true;
        }
        if seq.is_circle()? {
            has_circle = true;
        }
        if seq.is_bamboo()? {
            has_bamboo = true;
        }
    }
    for head in &hand.same2 {
        if head.is_character()? {
            has_character = true;
        }
        if head.is_circle()? {
            has_circle = true;
        }
        if head.is_bamboo()? {
            has_bamboo = true;
        }
        if head.has_honor()? {
            has_honor = true;
        }
    }

    if has_honor {
        return Ok((name, false, 0));
    }
    let suit_count = [has_character, has_circle, has_bamboo]
        .iter()
        .filter(|&&x| x)
        .count();
    if suit_count != 1 {
        return Ok((name, false, 0));
    }

    // 牌の数を集計して九蓮宝燈のパターンかチェック
    // 基本形: 1が3枚以上, 2~8が各1枚以上, 9が3枚以上
    let offset = if has_character {
        0
    } else if has_circle {
        9
    } else {
        18
    };
    let mut counts = [0u32; 9];
    for same in &hand.same3 {
        let t = same.get()[0];
        counts[(t - offset) as usize] += 3;
    }
    for seq in &hand.sequential3 {
        let tiles = seq.get();
        for t in &tiles {
            counts[(*t - offset) as usize] += 1;
        }
    }
    for head in &hand.same2 {
        let t = head.get()[0];
        counts[(t - offset) as usize] += 2;
    }
    for single in &hand.single {
        if *single >= offset && *single < offset + 9 {
            counts[(*single - offset) as usize] += 1;
        }
    }

    // 九蓮宝燈: 1が3枚以上、2~8が各1枚以上、9が3枚以上、合計14枚
    if counts[0] >= 3
        && counts[8] >= 3
        && counts[1] >= 1
        && counts[2] >= 1
        && counts[3] >= 1
        && counts[4] >= 1
        && counts[5] >= 1
        && counts[6] >= 1
        && counts[7] >= 1
    {
        let total: u32 = counts.iter().sum();
        if total == 14 {
            return Ok((name, true, 13));
        }
    }
    Ok((name, false, 0))
}
/// 四槓子
pub fn check_four_kans(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::FourKans,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 四槓子: 4つの槓子を持っている
    if status.kan_count == 4 {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// 天和
pub fn check_heavenly_hand(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HeavenlyHand,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 天和: 親の配牌時点で和了している（第一ツモ・親・自摸）
    if status.is_dealer && status.is_first_turn && status.is_self_picked && !status.has_claimed_open
    {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// 地和
pub fn check_hand_of_earth(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HandOfEarth,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 地和: 子の第一ツモで和了している（第一ツモ・子・自摸）
    if !status.is_dealer
        && status.is_first_turn
        && status.is_self_picked
        && !status.has_claimed_open
    {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;

    #[test]
    /// 国士無双で和了った
    fn test_win_by_thirteen_orphans() {
        let test_str = "19m19p19s1234567z 1m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_thirteen_orphans(&test_analyzer, &status, &settings).unwrap(),
            ("国士無双", true, 13)
        );
    }

    #[test]
    /// 四暗刻単騎で和了った
    fn test_win_by_four_concealed_triplets_single() {
        let test_str = "111333m444s1777z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        status.is_self_picked = true; // 自摸和了
        let settings = Settings::new();
        assert_eq!(
            check_four_concealed_triplets(&test_analyzer, &status, &settings).unwrap(),
            ("四暗刻", true, 13)
        );
    }

    #[test]
    /// 通常の四暗刻では、自摸和了のみ（ロンした場合は三暗刻＋対々和になる）
    fn test_not_win_by_four_concealed_triplets_single_if_not_self_pick() {
        let test_str = "111333m444s1777z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        status.is_self_picked = false;
        let settings = Settings::new();
        assert_eq!(
            check_four_concealed_triplets(&test_analyzer, &status, &settings).unwrap(),
            ("四暗刻", false, 0)
        );
    }
    #[test]
    /// 大三元で和了った
    fn test_win_by_big_three_dragons() {
        let test_str = "555666777z234m1p 1p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_big_three_dragons(&test_analyzer, &status, &settings).unwrap(),
            ("大三元", true, 13)
        );
    }
    #[test]
    /// 小四喜で和了った
    fn test_win_by_little_four_winds() {
        let test_str = "11122233344z23m 4m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_little_four_winds(&test_analyzer, &status, &settings).unwrap(),
            ("小四喜", true, 13)
        );
    }
    #[test]
    /// 大四喜で和了った
    fn test_win_by_big_four_winds() {
        let test_str = "5m 111z 222z 333z 444z 5m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_big_four_winds(&test_analyzer, &status, &settings).unwrap(),
            ("大四喜", true, 13)
        );
    }
    #[test]
    /// 字一色で和了った
    fn test_win_by_all_honors() {
        let test_str = "111222333z5z 777z 5z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_honors(&test_analyzer, &status, &settings).unwrap(),
            ("字一色", true, 13)
        );
    }
    #[test]
    /// 清老頭で和了った
    fn test_win_by_all_terminals() {
        let test_str = "111999m1p 111s 999p 1p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_terminals(&test_analyzer, &status, &settings).unwrap(),
            ("清老頭", true, 13)
        );
    }
    #[test]
    /// 緑一色で和了った
    fn test_win_by_all_green() {
        let test_str = "22233344s66z 888s 6z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_green(&test_analyzer, &status, &settings).unwrap(),
            ("緑一色", true, 13)
        );
    }
    #[test]
    /// 九蓮宝燈で和了った
    fn test_win_by_nine_gates() {
        let test_str = "1112345678999m 5m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_nine_gates(&test_analyzer, &status, &settings).unwrap(),
            ("九蓮宝燈", true, 13)
        );
    }
    #[test]
    /// 四槓子で和了った
    fn test_win_by_four_kans() {
        let test_str = "111333m444s1777z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.kan_count = 4;
        status.is_self_picked = true;
        assert_eq!(
            check_four_kans(&test_analyzer, &status, &settings).unwrap(),
            ("四槓子", true, 13)
        );
    }
    #[test]
    /// 天和で和了った
    fn test_win_by_heavenly_hand() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = true;
        status.is_first_turn = true;
        status.is_self_picked = true;
        assert_eq!(
            check_heavenly_hand(&test_analyzer, &status, &settings).unwrap(),
            ("天和", true, 13)
        );
    }
    #[test]
    /// 天和は子では成立しない（地和になる）
    fn test_not_win_by_heavenly_hand_if_not_dealer() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = false;
        status.is_first_turn = true;
        status.is_self_picked = true;
        assert_eq!(
            check_heavenly_hand(&test_analyzer, &status, &settings).unwrap(),
            ("天和", false, 0)
        );
    }
    #[test]
    /// 地和で和了った
    fn test_win_by_hand_of_earth() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = false;
        status.is_first_turn = true;
        status.is_self_picked = true;
        assert_eq!(
            check_hand_of_earth(&test_analyzer, &status, &settings).unwrap(),
            ("地和", true, 13)
        );
    }
    #[test]
    /// 地和は親では成立しない（天和になる）
    fn test_not_win_by_hand_of_earth_if_dealer() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = true;
        status.is_first_turn = true;
        status.is_self_picked = true;
        assert_eq!(
            check_hand_of_earth(&test_analyzer, &status, &settings).unwrap(),
            ("地和", false, 0)
        );
    }
}
