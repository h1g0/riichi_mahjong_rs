use anyhow::Result;

use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::winning_hand::name::*;

/// 二盃口
pub fn check_two_sets_of_identical_sequences(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::TwoSetsOfIdenticalSequences,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 門前でなければ二盃口は成立しない
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // 順子が4つなければ二盃口はありえない
    if hand_analyzer.sequential3.len() != 4 {
        return Ok((name, false, 0));
    }
    // 2組の同じ順子ペアがあるか確認
    let mut used = [false; 4];
    let mut pair_count = 0;
    for i in 0..4 {
        if used[i] {
            continue;
        }
        for j in (i + 1)..4 {
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
    if pair_count == 2 {
        Ok((name, true, 3))
    } else {
        Ok((name, false, 0))
    }
}
/// 純全帯么九
pub fn check_terminal_in_each_set(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::TerminalInEachSet,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 清老頭とは複合しないため、必ず順子が含まれる
    if hand_analyzer.sequential3.len() == 0 {
        return Ok((name, false, 0));
    }

    let mut no_1_9 = false;
    // 面子

    // 刻子
    for same in &hand_analyzer.same3 {
        if !same.has_1_or_9()? {
            no_1_9 = true;
        }
    }
    // 順子
    for seq in &hand_analyzer.sequential3 {
        if !seq.has_1_or_9()? {
            no_1_9 = true;
        }
    }

    // 雀頭
    for head in &hand_analyzer.same2 {
        if !head.has_1_or_9()? {
            no_1_9 = true;
        }
    }

    if no_1_9 {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        Ok((name, true, 2))
    } else {
        Ok((name, true, 3))
    }
}
/// 混一色
pub fn check_half_flush(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::HalfFlush,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    let mut has_honor = false;
    let mut has_character = false;
    let mut has_circle = false;
    let mut has_bamboo = false;

    for same in &hand_analyzer.same3 {
        if same.has_honor()? {
            has_honor = true;
        }
        if same.is_character()? {
            has_character = true;
        }
        if same.is_circle()? {
            has_circle = true;
        }
        if same.is_bamboo()? {
            has_bamboo = true;
        }
    }
    for seq in &hand_analyzer.sequential3 {
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
    for head in &hand_analyzer.same2 {
        if head.has_honor()? {
            has_honor = true;
        }
        if head.is_character()? {
            has_character = true;
        }
        if head.is_circle()? {
            has_circle = true;
        }
        if head.is_bamboo()? {
            has_bamboo = true;
        }
    }

    if !has_honor {
        return Ok((name, false, 0));
    }
    let suit_count = [has_character, has_circle, has_bamboo]
        .iter()
        .filter(|&&x| x)
        .count();
    if suit_count != 1 {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open {
        Ok((name, true, 2))
    } else {
        Ok((name, true, 3))
    }
}

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::*;
    use crate::winning_hand::check_2_han::*;

    #[test]
    /// 純全帯么九で和了った
    fn test_terminal_in_each_set() {
        let test_str = "123999m11p11179s 8s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_terminal_in_each_set(&test_analyzer, &status, &settings).unwrap(),
            ("純全帯么九", true, 3)
        );
    }
    #[test]
    /// 純全帯么九で和了った（食い下がり2翻）
    fn test_terminal_in_each_set_open() {
        let test_str = "123m111p7999s 789m 8s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_terminal_in_each_set(&test_analyzer, &status, &settings).unwrap(),
            ("純全帯么九（鳴）", true, 2)
        );
    }

    #[test]
    /// 混全帯么九は純全帯么九と複合しない
    fn test_terminal_or_honor_in_each_set_does_not_combined_with_terminal_in_each_set() {
        let test_str = "111789m111p99s11z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_terminal_or_honor_in_each_set(&test_analyzer, &status, &settings)
                .unwrap()
                .1,
            true
        );
        assert_eq!(
            check_terminal_in_each_set(&test_analyzer, &status, &settings)
                .unwrap()
                .1,
            false
        );
    }
    #[test]
    /// 純全帯么九は混全帯么九と複合しない
    fn test_terminal_in_each_set_does_not_combined_with_terminal_or_honor_in_each_set() {
        let test_str = "111789m111p1199s 9s";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_terminal_or_honor_in_each_set(&test_analyzer, &status, &settings)
                .unwrap()
                .1,
            false
        );
        assert_eq!(
            check_terminal_in_each_set(&test_analyzer, &status, &settings)
                .unwrap()
                .1,
            true
        );
    }
    #[test]
    /// 二盃口で和了った（高点法により七対子より二盃口が優先される）
    fn test_two_sets_of_identical_sequences() {
        let test_str = "112233m456456p7z 7z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(test_analyzer.form, Form::Normal);
        assert_eq!(
            check_two_sets_of_identical_sequences(&test_analyzer, &status, &settings).unwrap(),
            ("二盃口", true, 3)
        );
    }
    #[test]
    /// 混一色で和了った（食い下がり2翻）
    fn test_half_flush() {
        let test_str = "123456m2z 789m 111z 2z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_half_flush(&test_analyzer, &status, &settings).unwrap(),
            ("混一色（鳴）", true, 2)
        );
    }
    #[test]
    /// 混一色で和了った
    fn test_half_flush_closed() {
        let test_str = "11112345699m11z 9m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_half_flush(&test_analyzer, &status, &settings).unwrap(),
            ("混一色", true, 3)
        );
    }
}
