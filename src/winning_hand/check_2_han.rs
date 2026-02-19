use anyhow::Result;

use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::tile::{Dragon, Tile};
use crate::winning_hand::name::*;

/// 七対子
pub fn check_seven_pairs(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::SevenPairs,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    if hand_analyzer.form == Form::SevenPairs {
        Ok((name, true, 2))
    } else {
        Ok((name, false, 0))
    }
}

/// 三色同順
pub fn check_three_color_straight(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ThreeColorStraight,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 順子が3つ以上なければ三色同順はありえない
    if hand_analyzer.sequential3.len() < 3 {
        return Ok((name, false, 0));
    }
    for i in 0..hand_analyzer.sequential3.len() {
        for j in (i + 1)..hand_analyzer.sequential3.len() {
            for k in (j + 1)..hand_analyzer.sequential3.len() {
                let a = hand_analyzer.sequential3[i].get();
                let b = hand_analyzer.sequential3[j].get();
                let c = hand_analyzer.sequential3[k].get();
                // 3つの順子の開始牌が同じ数字（mod 9）で、かつ異なる色であること
                let a_num = a[0] % 9;
                let b_num = b[0] % 9;
                let c_num = c[0] % 9;
                if a_num == b_num && b_num == c_num {
                    let a_suit = a[0] / 9;
                    let b_suit = b[0] / 9;
                    let c_suit = c[0] / 9;
                    if a_suit != b_suit
                        && b_suit != c_suit
                        && a_suit != c_suit
                        && a_suit < 3
                        && b_suit < 3
                        && c_suit < 3
                    {
                        if status.has_claimed_open {
                            return Ok((name, true, 1));
                        } else {
                            return Ok((name, true, 2));
                        }
                    }
                }
            }
        }
    }
    Ok((name, false, 0))
}
/// 一気通貫
pub fn check_straight(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::Straight,
        status.has_claimed_open,
        settings.display_lang,
    );

    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    let mut m = [false; 3];
    let mut p = [false; 3];
    let mut s = [false; 3];

    for v in &hand_analyzer.sequential3 {
        match v.get() {
            [Tile::M1, Tile::M2, Tile::M3] => m[0] = true,
            [Tile::M4, Tile::M5, Tile::M6] => m[1] = true,
            [Tile::M7, Tile::M8, Tile::M9] => m[2] = true,
            [Tile::P1, Tile::P2, Tile::P3] => p[0] = true,
            [Tile::P4, Tile::P5, Tile::P6] => p[1] = true,
            [Tile::P7, Tile::P8, Tile::P9] => p[2] = true,
            [Tile::S1, Tile::S2, Tile::S3] => s[0] = true,
            [Tile::S4, Tile::S5, Tile::S6] => s[1] = true,
            [Tile::S7, Tile::S8, Tile::S9] => s[2] = true,
            _ => {}
        }
    }

    if (m[0] && m[1] && m[2]) || (p[0] && p[1] && p[2]) || (s[0] && s[1] && s[2]) {
        if status.has_claimed_open {
            return Ok((name, true, 1));
        } else {
            return Ok((name, true, 2));
        }
    }
    Ok((name, false, 0))
}
/// 対々和
pub fn check_all_triplet_hand(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllTripletHand,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    if hand_analyzer.same3.len() == 4 && hand_analyzer.same2.len() == 1 {
        return Ok((name, true, 2));
    }
    Ok((name, false, 0))
}
/// 三暗刻
pub fn check_three_closed_triplets(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ThreeClosedTriplets,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 刻子が3つ以上あれば三暗刻の可能性がある
    if hand_analyzer.same3.len() >= 3 {
        Ok((name, true, 2))
    } else {
        Ok((name, false, 0))
    }
}
/// 三色同刻
pub fn check_three_color_triplets(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ThreeColorTriplets,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 刻子が3つ以上なければ三色同刻はありえない
    if hand_analyzer.same3.len() < 3 {
        return Ok((name, false, 0));
    }
    for i in 0..hand_analyzer.same3.len() {
        for j in (i + 1)..hand_analyzer.same3.len() {
            for k in (j + 1)..hand_analyzer.same3.len() {
                let a = hand_analyzer.same3[i].get()[0];
                let b = hand_analyzer.same3[j].get()[0];
                let c = hand_analyzer.same3[k].get()[0];
                // 数牌のみ（字牌は三色同刻にならない）
                if a > Tile::S9 || b > Tile::S9 || c > Tile::S9 {
                    continue;
                }
                // 同じ数字（mod 9）で異なる色であること
                let a_num = a % 9;
                let b_num = b % 9;
                let c_num = c % 9;
                if a_num == b_num && b_num == c_num {
                    let a_suit = a / 9;
                    let b_suit = b / 9;
                    let c_suit = c / 9;
                    if a_suit != b_suit && b_suit != c_suit && a_suit != c_suit {
                        return Ok((name, true, 2));
                    }
                }
            }
        }
    }
    Ok((name, false, 0))
}
/// 混全帯么九
pub fn check_terminal_or_honor_in_each_set(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::TerminalOrHonorInEachSet,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }

    // 混老頭とは複合しないため、必ず順子が含まれる
    if hand_analyzer.sequential3.len() == 0 {
        return Ok((name, false, 0));
    }

    let mut no_1_9_honor = false;
    // 純全帯么九とは複合しないため、必ず三元牌が含まれる
    let mut has_honor = false;

    // 面子

    // 刻子
    for same in &hand_analyzer.same3 {
        if !same.has_1_or_9()? && !same.has_honor()? {
            no_1_9_honor = true;
        }

        if same.has_honor()? {
            has_honor = true;
        }
    }
    // 順子
    for seq in &hand_analyzer.sequential3 {
        if !seq.has_1_or_9()? {
            no_1_9_honor = true;
        }
    }

    // 雀頭
    for head in &hand_analyzer.same2 {
        if !head.has_1_or_9()? && !head.has_honor()? {
            no_1_9_honor = true;
        }
        if head.has_honor()? {
            has_honor = true;
        }
    }

    if no_1_9_honor || !has_honor {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        return Ok((name, true, 1));
    }
    Ok((name, true, 2))
}
/// 混老頭
pub fn check_all_terminals_and_honors(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllTerminalsAndHonors,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 混老頭は全ての面子・雀頭が么九牌（1,9）または字牌で構成される
    // 順子が含まれていてはいけない
    if hand_analyzer.sequential3.len() > 0 {
        return Ok((name, false, 0));
    }
    // 字牌が含まれていなければ清老頭であり混老頭にはならない
    let mut has_honor = false;
    // 数牌（1,9）が含まれていなければ字一色であり混老頭にはならない
    let mut has_terminal = false;

    for same in &hand_analyzer.same3 {
        if same.has_honor()? {
            has_honor = true;
        } else if same.has_1_or_9()? {
            has_terminal = true;
        } else {
            return Ok((name, false, 0));
        }
    }
    for head in &hand_analyzer.same2 {
        if head.has_honor()? {
            has_honor = true;
        } else if head.has_1_or_9()? {
            has_terminal = true;
        } else {
            return Ok((name, false, 0));
        }
    }
    if has_honor && has_terminal {
        Ok((name, true, 2))
    } else {
        Ok((name, false, 0))
    }
}
/// 小三元
pub fn check_little_three_dragons(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LittleThreeDragons,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 小三元: 三元牌のうち2つが刻子、1つが雀頭
    let mut dragon_triplet_count = 0;
    let mut dragon_pair = false;
    for same in &hand_analyzer.same3 {
        if same.has_dragon(Dragon::White)?
            || same.has_dragon(Dragon::Green)?
            || same.has_dragon(Dragon::Red)?
        {
            dragon_triplet_count += 1;
        }
    }
    for head in &hand_analyzer.same2 {
        if head.has_dragon(Dragon::White)?
            || head.has_dragon(Dragon::Green)?
            || head.has_dragon(Dragon::Red)?
        {
            dragon_pair = true;
        }
    }
    if dragon_triplet_count == 2 && dragon_pair {
        Ok((name, true, 2))
    } else {
        Ok((name, false, 0))
    }
}

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::*;
    #[test]
    /// 七対子で和了った
    fn test_win_by_seven_pairs() {
        let test_str = "1122m3344p5566s1z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_seven_pairs(&test_analyzer, &status, &settings).unwrap(),
            ("七対子", true, 2)
        );
    }
    #[test]
    /// 混全帯么九で和了った
    fn test_terminal_or_honor_in_each_set() {
        let test_str = "123999m111p79s44z 8s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_terminal_or_honor_in_each_set(&test_analyzer, &status, &settings).unwrap(),
            ("混全帯么九", true, 2)
        );
    }
    #[test]
    /// 混全帯么九で和了った（食い下がり1翻）
    fn test_terminal_or_honor_in_each_set_open() {
        let test_str = "123m111p79s44z 789m 8s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_terminal_or_honor_in_each_set(&test_analyzer, &status, &settings).unwrap(),
            ("混全帯么九（鳴）", true, 1)
        );
    }
    #[test]
    /// 対々和で和了った
    fn test_all_triplet_hand() {
        let test_str = "777m333p22z 555m 999s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_triplet_hand(&test_analyzer, &status, &settings).unwrap(),
            ("対々和", true, 2)
        );
    }

    #[test]
    /// 一気通貫で和了った
    fn test_straight() {
        let test_str = "123456789m78p22z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_straight(&test_analyzer, &status, &settings).unwrap(),
            ("一気通貫", true, 2)
        );
    }

    #[test]
    /// 一気通貫で和了った（食い下がり1翻）
    fn test_straight_open() {
        let test_str = "123m1p123s 456s 789s 1p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_straight(&test_analyzer, &status, &settings).unwrap(),
            ("一気通貫（鳴）", true, 1)
        );
    }
    #[test]
    /// 三色同順で和了った
    fn test_three_color_straight() {
        let test_str = "123m123p123s789m1z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_three_color_straight(&test_analyzer, &status, &settings).unwrap(),
            ("三色同順", true, 2)
        );
    }
    #[test]
    /// 三色同順で和了った（食い下がり1翻）
    fn test_three_color_straight_open() {
        let test_str = "123m123p1z 123s 789m 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_three_color_straight(&test_analyzer, &status, &settings).unwrap(),
            ("三色同順（鳴）", true, 1)
        );
    }
    #[test]
    /// 三暗刻で和了った
    fn test_three_closed_triplets() {
        let test_str = "111m333p999s789m1z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_three_closed_triplets(&test_analyzer, &status, &settings).unwrap(),
            ("三暗刻", true, 2)
        );
    }
    #[test]
    /// 三色同刻で和了った
    fn test_three_color_triplets() {
        let test_str = "111m111p111s789p5z 5z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_three_color_triplets(&test_analyzer, &status, &settings).unwrap(),
            ("三色同刻", true, 2)
        );
    }
    #[test]
    /// 混老頭で和了った
    fn test_all_terminals_and_honors() {
        let test_str = "111m999p1z 111z 999s 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_terminals_and_honors(&test_analyzer, &status, &settings).unwrap(),
            ("混老頭", true, 2)
        );
    }
    #[test]
    /// 小三元で和了った
    fn test_little_three_dragons() {
        let test_str = "555666z77z234m78p 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_little_three_dragons(&test_analyzer, &status, &settings).unwrap(),
            ("小三元", true, 2)
        );
    }
}
