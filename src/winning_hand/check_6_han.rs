use anyhow::Result;

use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::winning_hand::name::*;

/// 清一色
pub fn check_flush(
    hand: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(Kind::Flush, status.has_claimed_open, settings.display_lang);
    if !has_won(hand) {
        return Ok((name, false, 0));
    }
    // 清一色: 1種類の数牌のみで構成される（字牌なし）
    let mut has_honor = false;
    let mut has_character = false;
    let mut has_circle = false;
    let mut has_bamboo = false;

    for same in &hand.same3 {
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

    // 字牌があったら清一色ではない
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
    if status.has_claimed_open {
        Ok((name, true, 5))
    } else {
        Ok((name, true, 6))
    }
}

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::*;
    #[test]
    /// 清一色で和了った
    fn test_flush_closed() {
        let test_str = "1113456677778m 5m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_flush(&test_analyzer, &status, &settings).unwrap(),
            ("清一色", true, 6)
        );
    }
    #[test]
    /// 清一色で和了った（食い下がり5翻）
    fn test_flush_open() {
        let test_str = "1234569p 789p 111p 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = true;
        assert_eq!(
            check_flush(&test_analyzer, &status, &settings).unwrap(),
            ("清一色（鳴）", true, 5)
        );
    }
}
