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
    result.insert(
        Kind::ReadyHand,
        check_ready_hand(analyzer, status, settings)?,
    );
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
    result.insert(Kind::SelfPick, check_self_pick(analyzer, status, settings)?);
    // 一発
    result.insert(Kind::OneShot, check_one_shot(analyzer, status, settings)?);
    // 海底撈月
    result.insert(
        Kind::LastTileFromTheWall,
        check_last_tile_from_the_wall(analyzer, status, settings)?,
    );
    // 河底撈魚
    result.insert(
        Kind::LastDiscard,
        check_last_discard(analyzer, status, settings)?,
    );
    // 嶺上開花
    result.insert(
        Kind::DeadWallDraw,
        check_dead_wall_draw(analyzer, status, settings)?,
    );
    // 搶槓
    result.insert(
        Kind::RobbingAQuad,
        check_robbing_a_quad(analyzer, status, settings)?,
    );
    // ダブル立直
    result.insert(
        Kind::DoubleReady,
        check_double_ready(analyzer, status, settings)?,
    );
    // 平和
    result.insert(
        Kind::NoPointsHand,
        check_no_points_hand(analyzer, hand, status, settings)?,
    );
    // 一盃口
    result.insert(
        Kind::OneSetOfIdenticalSequences,
        check_one_set_of_identical_sequences(analyzer, status, settings)?,
    );
    // 三色同順
    result.insert(
        Kind::ThreeColorStraight,
        check_three_color_straight(analyzer, status, settings)?,
    );
    // 一気通貫
    result.insert(Kind::Straight, check_straight(analyzer, status, settings)?);
    // 二盃口
    result.insert(
        Kind::TwoSetsOfIdenticalSequences,
        check_two_sets_of_identical_sequences(analyzer, status, settings)?,
    );
    // 対々和
    result.insert(
        Kind::AllTripletHand,
        check_all_triplet_hand(analyzer, status, settings)?,
    );
    // 三暗刻
    result.insert(
        Kind::ThreeClosedTriplets,
        check_three_closed_triplets(analyzer, status, settings)?,
    );
    // 三色同刻
    result.insert(
        Kind::ThreeColorTriplets,
        check_three_color_triplets(analyzer, status, settings)?,
    );
    // 断么九
    result.insert(
        Kind::AllSimples,
        check_all_simples(analyzer, status, settings)?,
    );
    // 役牌（自風牌）
    result.insert(
        Kind::HonorTilesPlayersWind,
        check_honor_tiles_players_wind(analyzer, status, settings)?,
    );
    // 役牌（場風牌）
    result.insert(
        Kind::HonorTilesPrevailingWind,
        check_honor_tiles_prevailing_wind(analyzer, status, settings)?,
    );
    // 役牌（白）
    result.insert(
        Kind::HonorTilesWhiteDragon,
        check_honor_tiles_white_dragon(analyzer, status, settings)?,
    );
    // 役牌（發）
    result.insert(
        Kind::HonorTilesGreenDragon,
        check_honor_tiles_green_dragon(analyzer, status, settings)?,
    );
    // 役牌（中）
    result.insert(
        Kind::HonorTilesRedDragon,
        check_honor_tiles_red_dragon(analyzer, status, settings)?,
    );
    // 混全帯么九
    result.insert(
        Kind::TerminalOrHonorInEachSet,
        check_terminal_or_honor_in_each_set(analyzer, status, settings)?,
    );
    result.insert(
        Kind::TerminalInEachSet,
        check_terminal_in_each_set(analyzer, status, settings)?,
    );
    // 混老頭
    result.insert(
        Kind::AllTerminalsAndHonors,
        check_all_terminals_and_honors(analyzer, status, settings)?,
    );
    // 小三元
    result.insert(
        Kind::LittleThreeDragons,
        check_little_three_dragons(analyzer, status, settings)?,
        // 純全帯么九
    );
    // 混一色
    result.insert(
        Kind::HalfFlush,
        check_half_flush(analyzer, status, settings)?,
    );
    // 清一色
    result.insert(Kind::Flush, check_flush(analyzer, status, settings)?);
    // 国士無双
    result.insert(
        Kind::ThirteenOrphans,
        check_thirteen_orphans(analyzer, status, settings)?,
    );
    // 四暗刻
    result.insert(
        Kind::FourConcealedTriplets,
        check_four_concealed_triplets(analyzer, status, settings)?,
    );
    // 大三元
    result.insert(
        Kind::BigThreeDragons,
        check_big_three_dragons(analyzer, status, settings)?,
    );
    // 小四喜
    result.insert(
        Kind::LittleFourWinds,
        check_little_four_winds(analyzer, status, settings)?,
    );
    // 大四喜
    result.insert(
        Kind::BigFourWinds,
        check_big_four_winds(analyzer, status, settings)?,
    );
    // 字一色
    result.insert(
        Kind::AllHonors,
        check_all_honors(analyzer, status, settings)?,
    );
    // 清老頭
    result.insert(
        Kind::AllTerminals,
        check_all_terminals(analyzer, status, settings)?,
    );
    // 緑一色
    result.insert(Kind::AllGreen, check_all_green(analyzer, status, settings)?);
    // 九蓮宝燈
    result.insert(
        Kind::NineGates,
        check_nine_gates(analyzer, status, settings)?,
    );
    // 四槓子
    result.insert(Kind::FourKans, check_four_kans(analyzer, status, settings)?);
    // 天和
    result.insert(
        Kind::HeavenlyHand,
        check_heavenly_hand(analyzer, status, settings)?,
    );
    // 地和
    result.insert(
        Kind::HandOfEarth,
        check_hand_of_earth(analyzer, status, settings)?,
    );

    Ok(result)
}

/// ユニットテスト
#[cfg(test)]
mod tests {}
