use anyhow::Result;

use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::winning_hand::name::*;

/// Nagashi Mangan (流し満貫)
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
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Nagashi Mangan depends on the discard history, not the hand shape,
    // so it is decided by a status flag set by the server.
    if status.is_nagashi_mangan {
        Ok((name, true, 5))
    } else {
        Ok((name, false, 0))
    }
}

#[cfg(test)]
mod tests {}
