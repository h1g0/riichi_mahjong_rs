use anyhow::Result;
use std::collections::HashMap;
use strum::{EnumCount, IntoEnumIterator};

use crate::hand::Hand;
use crate::hand_info::hand_analyzer::HandAnalyzer;
use crate::hand_info::status::Status;
use crate::settings::*;
use crate::winning_hand::check_1_han::*;
use crate::winning_hand::check_2_han::*;
use crate::winning_hand::check_3_han::*;
use crate::winning_hand::check_5_han::*;
use crate::winning_hand::check_6_han::*;
use crate::winning_hand::check_yakuman::*;
use crate::winning_hand::name::*;

pub fn check(
    analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<HashMap<Kind, (&'static str, bool, u32)>> {
    let mut result = HashMap::with_capacity(Kind::COUNT);
    for hand_kind in Kind::iter() {
        result.insert(hand_kind, ("Unknown", false, 0));
    }

    // Nagashi Mangan settles an exhaustive draw rather than a tile-shape
    // win, so it cannot combine with ordinary yaku even if a caller supplies
    // a complete analyzer or unrelated status flags.
    if status.is_nagashi_mangan {
        result.insert(
            Kind::NagashiMangan,
            check_nagashi_mangan(analyzer, status, settings)?,
        );
        return Ok(result);
    }

    result.insert(Kind::Riichi, check_riichi(analyzer, status, settings)?);
    result.insert(
        Kind::SevenPairs,
        check_seven_pairs(analyzer, status, settings)?,
    );
    result.insert(
        Kind::NagashiMangan,
        check_nagashi_mangan(analyzer, status, settings)?,
    );
    result.insert(
        Kind::FullyConcealedHand,
        check_fully_concealed_hand(analyzer, status, settings)?,
    );
    result.insert(Kind::Unbroken, check_unbroken(analyzer, status, settings)?);
    result.insert(
        Kind::LastTileDraw,
        check_last_tile_draw(analyzer, status, settings)?,
    );
    result.insert(
        Kind::LastTileClaim,
        check_last_tile_claim(analyzer, status, settings)?,
    );
    result.insert(
        Kind::AfterAQuad,
        check_after_a_quad(analyzer, status, settings)?,
    );
    result.insert(
        Kind::RobbingAQuad,
        check_robbing_a_quad(analyzer, status, settings)?,
    );
    result.insert(
        Kind::DoubleRiichi,
        check_double_riichi(analyzer, status, settings)?,
    );
    result.insert(Kind::Pinfu, check_pinfu(analyzer, hand, status, settings)?);
    result.insert(
        Kind::TwinSequences,
        check_twin_sequences(analyzer, status, settings)?,
    );
    result.insert(
        Kind::MixedSequences,
        check_mixed_sequences(analyzer, status, settings)?,
    );
    result.insert(
        Kind::FullStraight,
        check_full_straight(analyzer, status, settings)?,
    );
    result.insert(
        Kind::DoubleTwinSequences,
        check_double_twin_sequences(analyzer, status, settings)?,
    );
    result.insert(
        Kind::AllTriplets,
        check_all_triplets(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ThreeConcealedTriplets,
        check_three_concealed_triplets(analyzer, hand, status, settings)?,
    );
    result.insert(
        Kind::MixedTriplets,
        check_mixed_triplets(analyzer, status, settings)?,
    );
    result.insert(
        Kind::AllInside,
        check_all_inside(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ValueHonourSeatWind,
        check_value_honour_seat_wind(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ValueHonourRoundWind,
        check_value_honour_round_wind(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ValueHonourWhiteDragon,
        check_value_honour_white_dragon(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ValueHonourGreenDragon,
        check_value_honour_green_dragon(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ValueHonourRedDragon,
        check_value_honour_red_dragon(analyzer, status, settings)?,
    );
    result.insert(
        Kind::CommonEnds,
        check_common_ends(analyzer, status, settings)?,
    );
    result.insert(
        Kind::PerfectEnds,
        check_perfect_ends(analyzer, status, settings)?,
    );
    result.insert(
        Kind::CommonTerminals,
        check_common_terminals(analyzer, status, settings)?,
    );
    result.insert(
        Kind::LittleDragons,
        check_little_dragons(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ThreeQuads,
        check_three_quads(analyzer, status, settings)?,
    );
    result.insert(
        Kind::CommonFlush,
        check_common_flush(analyzer, status, settings)?,
    );
    result.insert(
        Kind::PerfectFlush,
        check_perfect_flush(analyzer, status, settings)?,
    );
    result.insert(
        Kind::ThirteenOrphans,
        check_thirteen_orphans(analyzer, status, settings)?,
    );
    result.insert(
        Kind::FourConcealedTripletsPairWait,
        check_four_concealed_triplets_pair_wait(analyzer, hand, status, settings)?,
    );
    result.insert(
        Kind::FourConcealedTriplets,
        check_four_concealed_triplets(analyzer, hand, status, settings)?,
    );
    result.insert(
        Kind::BigDragons,
        check_big_dragons(analyzer, status, settings)?,
    );
    result.insert(
        Kind::LittleWinds,
        check_little_winds(analyzer, status, settings)?,
    );
    result.insert(Kind::BigWinds, check_big_winds(analyzer, status, settings)?);
    result.insert(
        Kind::AllHonours,
        check_all_honours(analyzer, status, settings)?,
    );
    result.insert(
        Kind::PerfectTerminals,
        check_perfect_terminals(analyzer, status, settings)?,
    );
    result.insert(Kind::AllGreen, check_all_green(analyzer, status, settings)?);
    result.insert(
        Kind::NineGates,
        check_nine_gates(analyzer, status, settings)?,
    );
    result.insert(
        Kind::FourQuads,
        check_four_quads(analyzer, status, settings)?,
    );
    result.insert(
        Kind::BlessingOfHeaven,
        check_blessing_of_heaven(analyzer, status, settings)?,
    );
    result.insert(
        Kind::BlessingOfEarth,
        check_blessing_of_earth(analyzer, status, settings)?,
    );

    Ok(result)
}

#[cfg(test)]
mod tests {}
