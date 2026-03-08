use anyhow::Result;

use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::winning_hand::name::*;

/// 流し満貫
pub fn check_nagashi_mangan(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::NagashiMangan,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !has_won(hand_analyzer) {
        return Ok((name, false, 0));
    }
    // 流し満貫は状態フラグで判定する
    if status.is_nagashi_mangan {
        Ok((name, true, 5))
    } else {
        Ok((name, false, 0))
    }
}

/// ユニットテスト
#[cfg(test)]
mod tests {}
