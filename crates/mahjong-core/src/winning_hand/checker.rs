use anyhow::Result;
/// 役を判定する
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

    // 立直
    result.insert(Kind::Riichi, check_riichi(analyzer, status, settings)?);
    // 七対子
    result.insert(
        Kind::SevenPairs,
        check_seven_pairs(analyzer, status, settings)?,
    );
    // 流し満貫
    result.insert(
        Kind::NagashiMangan,
        check_nagashi_mangan(analyzer, status, settings)?,
    );
    // 門前清自摸和
    result.insert(
        Kind::FullyConcealedHand,
        check_fully_concealed_hand(analyzer, status, settings)?,
    );
    // 一発
    result.insert(Kind::Unbroken, check_unbroken(analyzer, status, settings)?);
    // 海底撈月
    result.insert(
        Kind::LastTileDraw,
        check_last_tile_draw(analyzer, status, settings)?,
    );
    // 河底撈魚
    result.insert(
        Kind::LastTileClaim,
        check_last_tile_claim(analyzer, status, settings)?,
    );
    // 嶺上開花
    result.insert(
        Kind::AfterAQuad,
        check_after_a_quad(analyzer, status, settings)?,
    );
    // 搶槓
    result.insert(
        Kind::RobbingAQuad,
        check_robbing_a_quad(analyzer, status, settings)?,
    );
    // ダブル立直
    result.insert(
        Kind::DoubleRiichi,
        check_double_riichi(analyzer, status, settings)?,
    );
    // 平和
    result.insert(Kind::Pinfu, check_pinfu(analyzer, hand, status, settings)?);
    // 一盃口
    result.insert(
        Kind::TwinSequences,
        check_twin_sequences(analyzer, status, settings)?,
    );
    // 三色同順
    result.insert(
        Kind::MixedSequences,
        check_mixed_sequences(analyzer, status, settings)?,
    );
    // 一気通貫
    result.insert(
        Kind::FullStraight,
        check_full_straight(analyzer, status, settings)?,
    );
    // 二盃口
    result.insert(
        Kind::DoubleTwinSequences,
        check_double_twin_sequences(analyzer, status, settings)?,
    );
    // 対々和
    result.insert(
        Kind::AllTriplets,
        check_all_triplets(analyzer, status, settings)?,
    );
    // 三暗刻
    result.insert(
        Kind::ThreeConcealedTriplets,
        check_three_concealed_triplets(analyzer, hand, status, settings)?,
    );
    // 三色同刻
    result.insert(
        Kind::MixedTriplets,
        check_mixed_triplets(analyzer, status, settings)?,
    );
    // 断么九
    result.insert(
        Kind::AllInside,
        check_all_inside(analyzer, status, settings)?,
    );
    // 役牌（自風牌）
    result.insert(
        Kind::ValueHonourSeatWind,
        check_value_honour_seat_wind(analyzer, status, settings)?,
    );
    // 役牌（場風牌）
    result.insert(
        Kind::ValueHonourRoundWind,
        check_value_honour_round_wind(analyzer, status, settings)?,
    );
    // 役牌（白）
    result.insert(
        Kind::ValueHonourWhiteDragon,
        check_value_honour_white_dragon(analyzer, status, settings)?,
    );
    // 役牌（發）
    result.insert(
        Kind::ValueHonourGreenDragon,
        check_value_honour_green_dragon(analyzer, status, settings)?,
    );
    // 役牌（中）
    result.insert(
        Kind::ValueHonourRedDragon,
        check_value_honour_red_dragon(analyzer, status, settings)?,
    );
    // 混全帯么九
    result.insert(
        Kind::CommonEnds,
        check_common_ends(analyzer, status, settings)?,
    );
    // 純全帯么九
    result.insert(
        Kind::PerfectEnds,
        check_perfect_ends(analyzer, status, settings)?,
    );
    // 混老頭
    result.insert(
        Kind::CommonTerminals,
        check_common_terminals(analyzer, status, settings)?,
    );
    // 小三元
    result.insert(
        Kind::LittleDragons,
        check_little_dragons(analyzer, status, settings)?,
    );
    // 混一色
    result.insert(
        Kind::CommonFlush,
        check_common_flush(analyzer, status, settings)?,
    );
    // 清一色
    result.insert(
        Kind::PerfectFlush,
        check_perfect_flush(analyzer, status, settings)?,
    );
    // 国士無双
    result.insert(
        Kind::ThirteenOrphans,
        check_thirteen_orphans(analyzer, status, settings)?,
    );
    // 四暗刻単騎待ち
    result.insert(
        Kind::FourConcealedTripletsPairWait,
        check_four_concealed_triplets_pair_wait(analyzer, hand, status, settings)?,
    );
    // 四暗刻
    result.insert(
        Kind::FourConcealedTriplets,
        check_four_concealed_triplets(analyzer, hand, status, settings)?,
    );
    // 大三元
    result.insert(
        Kind::BigDragons,
        check_big_dragons(analyzer, status, settings)?,
    );
    // 小四喜
    result.insert(
        Kind::LittleWinds,
        check_little_winds(analyzer, status, settings)?,
    );
    // 大四喜
    result.insert(Kind::BigWinds, check_big_winds(analyzer, status, settings)?);
    // 字一色
    result.insert(
        Kind::AllHonours,
        check_all_honours(analyzer, status, settings)?,
    );
    // 清老頭
    result.insert(
        Kind::PerfectTerminals,
        check_perfect_terminals(analyzer, status, settings)?,
    );
    // 緑一色
    result.insert(Kind::AllGreen, check_all_green(analyzer, status, settings)?);
    // 九蓮宝燈
    result.insert(
        Kind::NineGates,
        check_nine_gates(analyzer, status, settings)?,
    );
    // 四槓子
    result.insert(
        Kind::FourQuads,
        check_four_quads(analyzer, status, settings)?,
    );
    // 天和
    result.insert(
        Kind::BlessingOfHeaven,
        check_blessing_of_heaven(analyzer, status, settings)?,
    );
    // 地和
    result.insert(
        Kind::BlessingOfEarth,
        check_blessing_of_earth(analyzer, status, settings)?,
    );

    Ok(result)
}

/// ユニットテスト
#[cfg(test)]
mod tests {}
