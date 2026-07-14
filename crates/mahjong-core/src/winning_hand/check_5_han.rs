use anyhow::Result;

use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::winning_hand::name::*;

/// Nagashi Mangan (流し満貫)
pub fn check_nagashi_mangan(
    _hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::NagashiMangan,
        status.has_claimed_open,
        settings.display_lang,
    );
    // Nagashi Mangan depends on the discard history, not the hand shape,
    // so it is decided by a status flag set by the server.
    if status.is_nagashi_mangan {
        Ok((name, true, 5))
    } else {
        Ok((name, false, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;

    #[test]
    fn nagashi_mangan_does_not_require_a_winning_hand_shape() {
        let hand = Hand::from("147m258p369s1234z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let mut status = Status::new();
        status.is_nagashi_mangan = true;

        assert!(!analyzer.shanten.has_won());
        assert_eq!(
            check_nagashi_mangan(&analyzer, &status, &Settings::new()).unwrap(),
            ("流し満貫", true, 5)
        );
    }
}
