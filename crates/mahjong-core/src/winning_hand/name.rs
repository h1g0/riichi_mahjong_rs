use serde::{Deserialize, Serialize};
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

use crate::settings::Lang;

/// Shape of a winning hand.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Form {
    /// Seven pairs (chiitoitsu / 七対子)
    SevenPairs,
    /// Thirteen orphans (kokushi musō / 国士無双)
    ThirteenOrphans,
    /// Standard four-groups-and-a-pair hand
    Normal,
}

/// A yaku (winning hand pattern).
///
/// English names follow WRC Rules 2025 (see docs/glossary.md).
/// The declaration order here also fixes the display order of equal-han
/// yaku on the result screen.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    EnumCountMacro,
    EnumIter,
    Serialize,
    Deserialize,
)]
pub enum Kind {
    /// Riichi (立直)
    Riichi,
    /// Double Riichi (ダブル立直)
    DoubleRiichi,
    /// Ippatsu (一発)
    Unbroken,
    /// Menzen Tsumo (門前清自摸和)
    FullyConcealedHand,
    /// Chiitoitsu (七対子)
    SevenPairs,
    /// Nagashi Mangan (流し満貫)
    NagashiMangan,
    /// Haitei (海底撈月)
    LastTileDraw,
    /// Hōtei (河底撈魚)
    LastTileClaim,
    /// Rinshan Kaihō (嶺上開花)
    AfterAQuad,
    /// Chankan (搶槓)
    RobbingAQuad,
    /// Pinfu (平和)
    Pinfu,
    /// Iipeikō (一盃口)
    TwinSequences,
    /// Sanshoku Dōjun (三色同順)
    MixedSequences,
    /// Ittsū (一気通貫)
    FullStraight,
    /// Ryanpeikō (二盃口)
    DoubleTwinSequences,
    /// Toitoi (対々和)
    AllTriplets,
    /// San'ankō (三暗刻)
    ThreeConcealedTriplets,
    /// Sanshoku Dōkō (三色同刻)
    MixedTriplets,
    /// Tan'yao (断么九)
    AllInside,
    /// Yakuhai: seat wind (役牌（自風牌）)
    ValueHonourSeatWind,
    /// Yakuhai: round wind (役牌（場風牌）)
    ValueHonourRoundWind,
    /// Yakuhai: White dragon (役牌（白）)
    ValueHonourWhiteDragon,
    /// Yakuhai: Green dragon (役牌（發）)
    ValueHonourGreenDragon,
    /// Yakuhai: Red dragon (役牌（中）)
    ValueHonourRedDragon,
    /// Chanta (混全帯么九)
    CommonEnds,
    /// Junchan (純全帯么九)
    PerfectEnds,
    /// Honrōtō (混老頭)
    CommonTerminals,
    /// Shōsangen (小三元)
    LittleDragons,
    /// Sankantsu (三槓子)
    ThreeQuads,
    /// Hon'itsu (混一色)
    CommonFlush,
    /// Chin'itsu (清一色)
    PerfectFlush,
    /// Kokushi Musō (国士無双)
    ThirteenOrphans,
    /// Kokushi Musō on a 13-sided wait (国士無双十三面待ち)
    ThirteenOrphansThirteenWait,
    /// Sūankō (四暗刻)
    FourConcealedTriplets,
    /// Sūankō tanki (四暗刻単騎待ち)
    FourConcealedTripletsPairWait,
    /// Daisangen (大三元)
    BigDragons,
    /// Shōsūshii (小四喜)
    LittleWinds,
    /// Daisūshii (大四喜)
    BigWinds,
    /// Tsūiisō (字一色)
    AllHonours,
    /// Chinrōtō (清老頭)
    PerfectTerminals,
    /// Ryūiisō (緑一色)
    AllGreen,
    /// Chūren Pōto (九蓮宝燈)
    NineGates,
    /// Pure Chūren Pōto (純正九蓮宝燈)
    PureNineGates,
    /// Sūkantsu (四槓子)
    FourQuads,
    /// Tenhō (天和)
    BlessingOfHeaven,
    /// Chihō (地和)
    BlessingOfEarth,
}

/// Returns the display name of a yaku.
///
/// # Arguments
/// * `hand_kind` - the yaku
/// * `has_opened` - whether the hand is open; yaku that lose han when open
///   get an "(Open)" /「（鳴）」suffix
/// * `lang` - display language
///
/// # Examples
///
/// ```
/// use mahjong_core::settings::Lang;
/// use mahjong_core::winning_hand::name::*;
///
/// assert_eq!(get(Kind::MixedSequences, true, Lang::Ja), "三色同順（鳴）");
/// assert_eq!(get(Kind::MixedSequences, false, Lang::Ja), "三色同順");
/// assert_eq!(get(Kind::MixedSequences, true, Lang::En), "Mixed Sequences (Open)");
/// assert_eq!(get(Kind::MixedSequences, false, Lang::En), "Mixed Sequences");
/// ```
pub fn get(hand_kind: Kind, has_openned: bool, lang: Lang) -> &'static str {
    match lang {
        Lang::En => get_en(hand_kind, has_openned),
        Lang::Ja => get_ja(hand_kind, has_openned),
    }
}

/// Appends the "(Open)" /「（鳴）」suffix for yaku that lose han when open.
macro_rules! openned_name {
    ($str:expr, $open:expr, $lang:expr) => {
        match $open {
            true => match $lang {
                Lang::En => concat!($str, " (Open)"),
                Lang::Ja => concat!($str, "（鳴）"),
            },
            _ => $str,
        }
    };
}

fn get_en(hand_kind: Kind, has_openned: bool) -> &'static str {
    // English names follow WRC Rules 2025 (see docs/glossary.md).
    match hand_kind {
        Kind::Riichi => "Riichi",
        Kind::SevenPairs => "Seven Pairs",
        Kind::NagashiMangan => "Nagashi Mangan",
        Kind::FullyConcealedHand => "Fully Concealed Hand",
        Kind::Unbroken => "Unbroken",
        Kind::LastTileDraw => "Last Tile Draw",
        Kind::LastTileClaim => "Last Tile Claim",
        Kind::AfterAQuad => "After a Quad",
        Kind::RobbingAQuad => "Robbing a Quad",
        Kind::DoubleRiichi => "Double Riichi",
        Kind::Pinfu => "Pinfu",
        Kind::TwinSequences => "Twin Sequences",
        Kind::MixedSequences => {
            openned_name!("Mixed Sequences", has_openned, Lang::En)
        }
        Kind::FullStraight => openned_name!("Full Straight", has_openned, Lang::En),

        Kind::DoubleTwinSequences => "Double Twin Sequences",
        Kind::AllTriplets => "All Triplets",
        Kind::ThreeConcealedTriplets => "Three Concealed Triplets",
        Kind::MixedTriplets => "Mixed Triplets",
        Kind::AllInside => "All Inside",
        Kind::ValueHonourSeatWind => "Value Honour (seat wind)",
        Kind::ValueHonourRoundWind => "Value Honour (round wind)",
        Kind::ValueHonourWhiteDragon => "Value Honour (White dragon)",
        Kind::ValueHonourGreenDragon => "Value Honour (Green dragon)",
        Kind::ValueHonourRedDragon => "Value Honour (Red dragon)",
        Kind::CommonEnds => {
            openned_name!("Common Ends", has_openned, Lang::En)
        }
        Kind::PerfectEnds => {
            openned_name!("Perfect Ends", has_openned, Lang::En)
        }
        Kind::CommonTerminals => "Common Terminals",
        Kind::LittleDragons => "Little Dragons",
        Kind::ThreeQuads => "Three Quads",
        Kind::CommonFlush => {
            openned_name!("Common Flush", has_openned, Lang::En)
        }
        Kind::PerfectFlush => {
            openned_name!("Perfect Flush", has_openned, Lang::En)
        }
        Kind::ThirteenOrphans => "Thirteen Orphans",
        Kind::ThirteenOrphansThirteenWait => "Thirteen Orphans (13-sided wait)",
        Kind::FourConcealedTriplets => "Four Concealed Triplets",
        Kind::FourConcealedTripletsPairWait => "Four Concealed Triplets (pair wait)",
        Kind::BigDragons => "Big Dragons",
        Kind::LittleWinds => "Little Winds",
        Kind::BigWinds => "Big Winds",
        Kind::AllHonours => "All Honours",
        Kind::PerfectTerminals => "Perfect Terminals",
        Kind::AllGreen => "All Green",
        Kind::NineGates => "Nine Gates",
        Kind::PureNineGates => "Pure Nine Gates",
        Kind::FourQuads => "Four Quads",
        Kind::BlessingOfHeaven => "Blessing of Heaven",
        Kind::BlessingOfEarth => "Blessing of Earth",
    }
}

fn get_ja(hand_kind: Kind, has_openned: bool) -> &'static str {
    match hand_kind {
        Kind::Riichi => "立直",
        Kind::SevenPairs => "七対子",
        Kind::NagashiMangan => "流し満貫",
        Kind::FullyConcealedHand => "門前清自摸和",
        Kind::Unbroken => "一発",
        Kind::LastTileDraw => "海底撈月",
        Kind::LastTileClaim => "河底撈魚",
        Kind::AfterAQuad => "嶺上開花",
        Kind::RobbingAQuad => "搶槓",
        Kind::DoubleRiichi => "ダブル立直",
        Kind::Pinfu => "平和",
        Kind::TwinSequences => "一盃口",
        Kind::MixedSequences => {
            openned_name!("三色同順", has_openned, Lang::Ja)
        }
        Kind::FullStraight => {
            openned_name!("一気通貫", has_openned, Lang::Ja)
        }
        Kind::DoubleTwinSequences => "二盃口",
        Kind::AllTriplets => "対々和",
        Kind::ThreeConcealedTriplets => "三暗刻",
        Kind::MixedTriplets => "三色同刻",
        Kind::AllInside => "断么九",
        Kind::ValueHonourSeatWind => "役牌（自風牌）",
        Kind::ValueHonourRoundWind => "役牌（場風牌）",
        Kind::ValueHonourWhiteDragon => "役牌（白）",
        Kind::ValueHonourGreenDragon => "役牌（發）",
        Kind::ValueHonourRedDragon => "役牌（中）",
        Kind::CommonEnds => {
            openned_name!("混全帯么九", has_openned, Lang::Ja)
        }
        Kind::PerfectEnds => {
            openned_name!("純全帯么九", has_openned, Lang::Ja)
        }
        Kind::CommonTerminals => "混老頭",
        Kind::LittleDragons => "小三元",
        Kind::ThreeQuads => "三槓子",
        Kind::CommonFlush => {
            openned_name!("混一色", has_openned, Lang::Ja)
        }
        Kind::PerfectFlush => {
            openned_name!("清一色", has_openned, Lang::Ja)
        }
        Kind::ThirteenOrphans => "国士無双",
        Kind::ThirteenOrphansThirteenWait => "国士無双十三面待ち",
        Kind::FourConcealedTriplets => "四暗刻",
        Kind::FourConcealedTripletsPairWait => "四暗刻単騎待ち",
        Kind::BigDragons => "大三元",
        Kind::LittleWinds => "小四喜",
        Kind::BigWinds => "大四喜",
        Kind::AllHonours => "字一色",
        Kind::PerfectTerminals => "清老頭",
        Kind::AllGreen => "緑一色",
        Kind::NineGates => "九蓮宝燈",
        Kind::PureNineGates => "純正九蓮宝燈",
        Kind::FourQuads => "四槓子",
        Kind::BlessingOfHeaven => "天和",
        Kind::BlessingOfEarth => "地和",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- get() dispatch ---

    #[test]
    fn get_dispatches_to_en() {
        assert_eq!(get(Kind::Riichi, false, Lang::En), "Riichi");
    }

    #[test]
    fn get_dispatches_to_ja() {
        assert_eq!(get(Kind::Riichi, false, Lang::Ja), "立直");
    }

    // --- English names (closed) ---

    #[test]
    fn en_closed_all_variants() {
        let cases: Vec<(Kind, &str)> = vec![
            (Kind::Riichi, "Riichi"),
            (Kind::DoubleRiichi, "Double Riichi"),
            (Kind::Unbroken, "Unbroken"),
            (Kind::FullyConcealedHand, "Fully Concealed Hand"),
            (Kind::SevenPairs, "Seven Pairs"),
            (Kind::NagashiMangan, "Nagashi Mangan"),
            (Kind::LastTileDraw, "Last Tile Draw"),
            (Kind::LastTileClaim, "Last Tile Claim"),
            (Kind::AfterAQuad, "After a Quad"),
            (Kind::RobbingAQuad, "Robbing a Quad"),
            (Kind::Pinfu, "Pinfu"),
            (Kind::TwinSequences, "Twin Sequences"),
            (Kind::MixedSequences, "Mixed Sequences"),
            (Kind::FullStraight, "Full Straight"),
            (Kind::DoubleTwinSequences, "Double Twin Sequences"),
            (Kind::AllTriplets, "All Triplets"),
            (Kind::ThreeConcealedTriplets, "Three Concealed Triplets"),
            (Kind::MixedTriplets, "Mixed Triplets"),
            (Kind::AllInside, "All Inside"),
            (Kind::ValueHonourSeatWind, "Value Honour (seat wind)"),
            (Kind::ValueHonourRoundWind, "Value Honour (round wind)"),
            (Kind::ValueHonourWhiteDragon, "Value Honour (White dragon)"),
            (Kind::ValueHonourGreenDragon, "Value Honour (Green dragon)"),
            (Kind::ValueHonourRedDragon, "Value Honour (Red dragon)"),
            (Kind::CommonEnds, "Common Ends"),
            (Kind::PerfectEnds, "Perfect Ends"),
            (Kind::CommonTerminals, "Common Terminals"),
            (Kind::LittleDragons, "Little Dragons"),
            (Kind::ThreeQuads, "Three Quads"),
            (Kind::CommonFlush, "Common Flush"),
            (Kind::PerfectFlush, "Perfect Flush"),
            (Kind::ThirteenOrphans, "Thirteen Orphans"),
            (
                Kind::ThirteenOrphansThirteenWait,
                "Thirteen Orphans (13-sided wait)",
            ),
            (Kind::FourConcealedTriplets, "Four Concealed Triplets"),
            (
                Kind::FourConcealedTripletsPairWait,
                "Four Concealed Triplets (pair wait)",
            ),
            (Kind::BigDragons, "Big Dragons"),
            (Kind::LittleWinds, "Little Winds"),
            (Kind::BigWinds, "Big Winds"),
            (Kind::AllHonours, "All Honours"),
            (Kind::PerfectTerminals, "Perfect Terminals"),
            (Kind::AllGreen, "All Green"),
            (Kind::NineGates, "Nine Gates"),
            (Kind::PureNineGates, "Pure Nine Gates"),
            (Kind::FourQuads, "Four Quads"),
            (Kind::BlessingOfHeaven, "Blessing of Heaven"),
            (Kind::BlessingOfEarth, "Blessing of Earth"),
        ];
        for (kind, expected) in cases {
            let label = format!("{kind:?}");
            assert_eq!(get(kind, false, Lang::En), expected, "kind: {label}");
        }
    }

    // --- English names (open) — only openable yaku change ---

    #[test]
    fn en_open_openable_yaku() {
        let cases: Vec<(Kind, &str)> = vec![
            (Kind::MixedSequences, "Mixed Sequences (Open)"),
            (Kind::FullStraight, "Full Straight (Open)"),
            (Kind::CommonEnds, "Common Ends (Open)"),
            (Kind::PerfectEnds, "Perfect Ends (Open)"),
            (Kind::CommonFlush, "Common Flush (Open)"),
            (Kind::PerfectFlush, "Perfect Flush (Open)"),
        ];
        for (kind, expected) in cases {
            let label = format!("{kind:?}");
            assert_eq!(get(kind, true, Lang::En), expected, "kind: {label}");
        }
    }

    #[test]
    fn en_open_non_openable_yaku_unchanged() {
        // Yaku whose name does not change when has_opened=true
        let cases: Vec<(Kind, &str)> = vec![
            (Kind::Riichi, "Riichi"),
            (Kind::DoubleRiichi, "Double Riichi"),
            (Kind::Unbroken, "Unbroken"),
            (Kind::FullyConcealedHand, "Fully Concealed Hand"),
            (Kind::SevenPairs, "Seven Pairs"),
            (Kind::NagashiMangan, "Nagashi Mangan"),
            (Kind::LastTileDraw, "Last Tile Draw"),
            (Kind::LastTileClaim, "Last Tile Claim"),
            (Kind::AfterAQuad, "After a Quad"),
            (Kind::RobbingAQuad, "Robbing a Quad"),
            (Kind::Pinfu, "Pinfu"),
            (Kind::TwinSequences, "Twin Sequences"),
            (Kind::DoubleTwinSequences, "Double Twin Sequences"),
            (Kind::AllTriplets, "All Triplets"),
            (Kind::ThreeConcealedTriplets, "Three Concealed Triplets"),
            (Kind::MixedTriplets, "Mixed Triplets"),
            (Kind::AllInside, "All Inside"),
            (Kind::ValueHonourSeatWind, "Value Honour (seat wind)"),
            (Kind::ValueHonourRoundWind, "Value Honour (round wind)"),
            (Kind::ValueHonourWhiteDragon, "Value Honour (White dragon)"),
            (Kind::ValueHonourGreenDragon, "Value Honour (Green dragon)"),
            (Kind::ValueHonourRedDragon, "Value Honour (Red dragon)"),
            (Kind::CommonTerminals, "Common Terminals"),
            (Kind::LittleDragons, "Little Dragons"),
            (Kind::ThreeQuads, "Three Quads"),
            (Kind::ThirteenOrphans, "Thirteen Orphans"),
            (
                Kind::ThirteenOrphansThirteenWait,
                "Thirteen Orphans (13-sided wait)",
            ),
            (Kind::FourConcealedTriplets, "Four Concealed Triplets"),
            (
                Kind::FourConcealedTripletsPairWait,
                "Four Concealed Triplets (pair wait)",
            ),
            (Kind::BigDragons, "Big Dragons"),
            (Kind::LittleWinds, "Little Winds"),
            (Kind::BigWinds, "Big Winds"),
            (Kind::AllHonours, "All Honours"),
            (Kind::PerfectTerminals, "Perfect Terminals"),
            (Kind::AllGreen, "All Green"),
            (Kind::NineGates, "Nine Gates"),
            (Kind::PureNineGates, "Pure Nine Gates"),
            (Kind::FourQuads, "Four Quads"),
            (Kind::BlessingOfHeaven, "Blessing of Heaven"),
            (Kind::BlessingOfEarth, "Blessing of Earth"),
        ];
        for (kind, expected) in cases {
            let label = format!("{kind:?}");
            assert_eq!(get(kind, true, Lang::En), expected, "kind: {label}");
        }
    }

    // --- Japanese names (closed) ---

    #[test]
    fn ja_closed_all_variants() {
        let cases: Vec<(Kind, &str)> = vec![
            (Kind::Riichi, "立直"),
            (Kind::DoubleRiichi, "ダブル立直"),
            (Kind::Unbroken, "一発"),
            (Kind::FullyConcealedHand, "門前清自摸和"),
            (Kind::SevenPairs, "七対子"),
            (Kind::NagashiMangan, "流し満貫"),
            (Kind::LastTileDraw, "海底撈月"),
            (Kind::LastTileClaim, "河底撈魚"),
            (Kind::AfterAQuad, "嶺上開花"),
            (Kind::RobbingAQuad, "搶槓"),
            (Kind::Pinfu, "平和"),
            (Kind::TwinSequences, "一盃口"),
            (Kind::MixedSequences, "三色同順"),
            (Kind::FullStraight, "一気通貫"),
            (Kind::DoubleTwinSequences, "二盃口"),
            (Kind::AllTriplets, "対々和"),
            (Kind::ThreeConcealedTriplets, "三暗刻"),
            (Kind::MixedTriplets, "三色同刻"),
            (Kind::AllInside, "断么九"),
            (Kind::ValueHonourSeatWind, "役牌（自風牌）"),
            (Kind::ValueHonourRoundWind, "役牌（場風牌）"),
            (Kind::ValueHonourWhiteDragon, "役牌（白）"),
            (Kind::ValueHonourGreenDragon, "役牌（發）"),
            (Kind::ValueHonourRedDragon, "役牌（中）"),
            (Kind::CommonEnds, "混全帯么九"),
            (Kind::PerfectEnds, "純全帯么九"),
            (Kind::CommonTerminals, "混老頭"),
            (Kind::LittleDragons, "小三元"),
            (Kind::ThreeQuads, "三槓子"),
            (Kind::CommonFlush, "混一色"),
            (Kind::PerfectFlush, "清一色"),
            (Kind::ThirteenOrphans, "国士無双"),
            (Kind::ThirteenOrphansThirteenWait, "国士無双十三面待ち"),
            (Kind::FourConcealedTriplets, "四暗刻"),
            (Kind::FourConcealedTripletsPairWait, "四暗刻単騎待ち"),
            (Kind::BigDragons, "大三元"),
            (Kind::LittleWinds, "小四喜"),
            (Kind::BigWinds, "大四喜"),
            (Kind::AllHonours, "字一色"),
            (Kind::PerfectTerminals, "清老頭"),
            (Kind::AllGreen, "緑一色"),
            (Kind::NineGates, "九蓮宝燈"),
            (Kind::PureNineGates, "純正九蓮宝燈"),
            (Kind::FourQuads, "四槓子"),
            (Kind::BlessingOfHeaven, "天和"),
            (Kind::BlessingOfEarth, "地和"),
        ];
        for (kind, expected) in cases {
            let label = format!("{kind:?}");
            assert_eq!(get(kind, false, Lang::Ja), expected, "kind: {label}");
        }
    }

    // --- Japanese names (open) ---

    #[test]
    fn ja_open_openable_yaku() {
        let cases: Vec<(Kind, &str)> = vec![
            (Kind::MixedSequences, "三色同順（鳴）"),
            (Kind::FullStraight, "一気通貫（鳴）"),
            (Kind::CommonEnds, "混全帯么九（鳴）"),
            (Kind::PerfectEnds, "純全帯么九（鳴）"),
            (Kind::CommonFlush, "混一色（鳴）"),
            (Kind::PerfectFlush, "清一色（鳴）"),
        ];
        for (kind, expected) in cases {
            let label = format!("{kind:?}");
            assert_eq!(get(kind, true, Lang::Ja), expected, "kind: {label}");
        }
    }

    #[test]
    fn ja_open_non_openable_yaku_unchanged() {
        let cases: Vec<(Kind, &str)> = vec![
            (Kind::Riichi, "立直"),
            (Kind::DoubleRiichi, "ダブル立直"),
            (Kind::Unbroken, "一発"),
            (Kind::FullyConcealedHand, "門前清自摸和"),
            (Kind::SevenPairs, "七対子"),
            (Kind::NagashiMangan, "流し満貫"),
            (Kind::LastTileDraw, "海底撈月"),
            (Kind::LastTileClaim, "河底撈魚"),
            (Kind::AfterAQuad, "嶺上開花"),
            (Kind::RobbingAQuad, "搶槓"),
            (Kind::Pinfu, "平和"),
            (Kind::TwinSequences, "一盃口"),
            (Kind::DoubleTwinSequences, "二盃口"),
            (Kind::AllTriplets, "対々和"),
            (Kind::ThreeConcealedTriplets, "三暗刻"),
            (Kind::MixedTriplets, "三色同刻"),
            (Kind::AllInside, "断么九"),
            (Kind::ValueHonourSeatWind, "役牌（自風牌）"),
            (Kind::ValueHonourRoundWind, "役牌（場風牌）"),
            (Kind::ValueHonourWhiteDragon, "役牌（白）"),
            (Kind::ValueHonourGreenDragon, "役牌（發）"),
            (Kind::ValueHonourRedDragon, "役牌（中）"),
            (Kind::CommonTerminals, "混老頭"),
            (Kind::LittleDragons, "小三元"),
            (Kind::ThreeQuads, "三槓子"),
            (Kind::ThirteenOrphans, "国士無双"),
            (Kind::ThirteenOrphansThirteenWait, "国士無双十三面待ち"),
            (Kind::FourConcealedTriplets, "四暗刻"),
            (Kind::FourConcealedTripletsPairWait, "四暗刻単騎待ち"),
            (Kind::BigDragons, "大三元"),
            (Kind::LittleWinds, "小四喜"),
            (Kind::BigWinds, "大四喜"),
            (Kind::AllHonours, "字一色"),
            (Kind::PerfectTerminals, "清老頭"),
            (Kind::AllGreen, "緑一色"),
            (Kind::NineGates, "九蓮宝燈"),
            (Kind::PureNineGates, "純正九蓮宝燈"),
            (Kind::FourQuads, "四槓子"),
            (Kind::BlessingOfHeaven, "天和"),
            (Kind::BlessingOfEarth, "地和"),
        ];
        for (kind, expected) in cases {
            let label = format!("{kind:?}");
            assert_eq!(get(kind, true, Lang::Ja), expected, "kind: {label}");
        }
    }
}
