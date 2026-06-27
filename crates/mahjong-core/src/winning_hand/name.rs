use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

use crate::settings::Lang;

/// 和了時の手牌の形態
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Form {
    /// 七対子
    SevenPairs,
    /// 国士無双
    ThirteenOrphans,
    /// 通常（4面子1雀頭）の手牌
    Normal,
}

/// 和了役を表す列挙型
///
/// 英語名は WRC Rules 2025 に準拠する（docs/glossary.md を参照）
/// ここでの定義順で同翻役のリザルト画面の役の表示順も決定する
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, EnumCountMacro, EnumIter)]
pub enum Kind {
    /// 立直
    Riichi,
    /// ダブル立直
    DoubleRiichi,
    /// 一発
    Unbroken,
    /// 門前清自摸和
    FullyConcealedHand,
    /// 七対子
    SevenPairs,
    /// 流し満貫
    NagashiMangan,
    /// 海底撈月
    LastTileDraw,
    /// 河底撈魚
    LastTileClaim,
    /// 嶺上開花
    AfterAQuad,
    /// 搶槓
    RobbingAQuad,
    /// 平和
    Pinfu,
    /// 一盃口
    TwinSequences,
    /// 三色同順
    MixedSequences,
    /// 一気通貫
    FullStraight,
    /// 二盃口
    DoubleTwinSequences,
    /// 対々和
    AllTriplets,
    /// 三暗刻
    ThreeConcealedTriplets,
    /// 三色同刻
    MixedTriplets,
    /// 断么九
    AllInside,
    /// 役牌（自風牌）
    ValueHonourSeatWind,
    /// 役牌（場風牌）
    ValueHonourRoundWind,
    /// 役牌（白）
    ValueHonourWhiteDragon,
    /// 役牌（發）
    ValueHonourGreenDragon,
    /// 役牌（中）
    ValueHonourRedDragon,
    /// 混全帯么九
    CommonEnds,
    /// 純全帯么九
    PerfectEnds,
    /// 混老頭
    CommonTerminals,
    /// 小三元
    LittleDragons,
    /// 混一色
    CommonFlush,
    /// 清一色
    PerfectFlush,
    /// 国士無双
    ThirteenOrphans,
    /// 四暗刻
    FourConcealedTriplets,
    /// 四暗刻単騎待ち
    FourConcealedTripletsPairWait,
    /// 大三元
    BigDragons,
    /// 小四喜
    LittleWinds,
    /// 大四喜
    BigWinds,
    /// 字一色
    AllHonours,
    /// 清老頭
    PerfectTerminals,
    /// 緑一色
    AllGreen,
    /// 九蓮宝燈
    NineGates,
    /// 四槓子
    FourQuads,
    /// 天和
    BlessingOfHeaven,
    /// 地和
    BlessingOfEarth,
}

/// 和了役の名前を返す
///
/// # Arguments
/// * `hand_kind` - 和了役の種類
/// * `has_opened` - 副露しているか否か（喰い下がり役は`true`にすると名前の後に「（鳴）」が付く）
/// * `lang` - 言語
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

/// 喰い下がり役に対しては「（鳴）」を付けるマクロ
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
    // 英語名は WRC Rules 2025 に準拠する（docs/glossary.md を参照）
    match hand_kind {
        // 立直
        Kind::Riichi => "Riichi",
        // 七対子
        Kind::SevenPairs => "Seven Pairs",
        // 流し満貫
        Kind::NagashiMangan => "Nagashi Mangan",
        // 門前清自摸和
        Kind::FullyConcealedHand => "Fully Concealed Hand",
        // 一発
        Kind::Unbroken => "Unbroken",
        // 海底撈月
        Kind::LastTileDraw => "Last Tile Draw",
        // 河底撈魚
        Kind::LastTileClaim => "Last Tile Claim",
        // 嶺上開花
        Kind::AfterAQuad => "After a Quad",
        // 搶槓
        Kind::RobbingAQuad => "Robbing a Quad",
        // ダブル立直
        Kind::DoubleRiichi => "Double Riichi",
        // 平和
        Kind::Pinfu => "Pinfu",
        // 一盃口
        Kind::TwinSequences => "Twin Sequences",
        // 三色同順
        Kind::MixedSequences => {
            openned_name!("Mixed Sequences", has_openned, Lang::En)
        }
        // 一気通貫
        Kind::FullStraight => openned_name!("Full Straight", has_openned, Lang::En),

        // 二盃口
        Kind::DoubleTwinSequences => "Double Twin Sequences",
        // 対々和
        Kind::AllTriplets => "All Triplets",
        // 三暗刻
        Kind::ThreeConcealedTriplets => "Three Concealed Triplets",
        // 三色同刻
        Kind::MixedTriplets => "Mixed Triplets",
        // 断么九
        Kind::AllInside => "All Inside",
        // 役牌（自風牌）
        Kind::ValueHonourSeatWind => "Value Honour (seat wind)",
        // 役牌（場風牌）
        Kind::ValueHonourRoundWind => "Value Honour (round wind)",
        // 役牌（白）
        Kind::ValueHonourWhiteDragon => "Value Honour (White dragon)",
        // 役牌（發）
        Kind::ValueHonourGreenDragon => "Value Honour (Green dragon)",
        // 役牌（中）
        Kind::ValueHonourRedDragon => "Value Honour (Red dragon)",
        // 混全帯么九
        Kind::CommonEnds => {
            openned_name!("Common Ends", has_openned, Lang::En)
        }
        // 純全帯么九
        Kind::PerfectEnds => {
            openned_name!("Perfect Ends", has_openned, Lang::En)
        }
        // 混老頭
        Kind::CommonTerminals => "Common Terminals",
        // 小三元
        Kind::LittleDragons => "Little Dragons",
        // 混一色
        Kind::CommonFlush => {
            openned_name!("Common Flush", has_openned, Lang::En)
        }
        // 清一色
        Kind::PerfectFlush => {
            openned_name!("Perfect Flush", has_openned, Lang::En)
        }
        // 国士無双
        Kind::ThirteenOrphans => "Thirteen Orphans",
        // 四暗刻
        Kind::FourConcealedTriplets => "Four Concealed Triplets",
        // 四暗刻単騎待ち
        Kind::FourConcealedTripletsPairWait => "Four Concealed Triplets (pair wait)",
        // 大三元
        Kind::BigDragons => "Big Dragons",
        // 小四喜
        Kind::LittleWinds => "Little Winds",
        // 大四喜
        Kind::BigWinds => "Big Winds",
        // 字一色
        Kind::AllHonours => "All Honours",
        // 清老頭
        Kind::PerfectTerminals => "Perfect Terminals",
        // 緑一色
        Kind::AllGreen => "All Green",
        // 九蓮宝燈
        Kind::NineGates => "Nine Gates",
        // 四槓子
        Kind::FourQuads => "Four Quads",
        // 天和
        Kind::BlessingOfHeaven => "Blessing of Heaven",
        // 地和
        Kind::BlessingOfEarth => "Blessing of Earth",
    }
}

fn get_ja(hand_kind: Kind, has_openned: bool) -> &'static str {
    match hand_kind {
        // 立直
        Kind::Riichi => "立直",
        // 七対子
        Kind::SevenPairs => "七対子",
        // 流し満貫
        Kind::NagashiMangan => "流し満貫",
        // 門前清自摸和
        Kind::FullyConcealedHand => "門前清自摸和",
        // 一発
        Kind::Unbroken => "一発",
        // 海底撈月
        Kind::LastTileDraw => "海底撈月",
        // 河底撈魚
        Kind::LastTileClaim => "河底撈魚",
        // 嶺上開花
        Kind::AfterAQuad => "嶺上開花",
        // 搶槓
        Kind::RobbingAQuad => "搶槓",
        // ダブル立直
        Kind::DoubleRiichi => "ダブル立直",
        // 平和
        Kind::Pinfu => "平和",
        // 一盃口
        Kind::TwinSequences => "一盃口",
        // 三色同順
        Kind::MixedSequences => {
            openned_name!("三色同順", has_openned, Lang::Ja)
        }
        // 一気通貫
        Kind::FullStraight => {
            openned_name!("一気通貫", has_openned, Lang::Ja)
        }
        // 二盃口
        Kind::DoubleTwinSequences => "二盃口",
        // 対々和
        Kind::AllTriplets => "対々和",
        // 三暗刻
        Kind::ThreeConcealedTriplets => "三暗刻",
        // 三色同刻
        Kind::MixedTriplets => "三色同刻",
        // 断么九
        Kind::AllInside => "断么九",
        // 役牌（自風牌）
        Kind::ValueHonourSeatWind => "役牌（自風牌）",
        // 役牌（場風牌）
        Kind::ValueHonourRoundWind => "役牌（場風牌）",
        // 役牌（白）
        Kind::ValueHonourWhiteDragon => "役牌（白）",
        // 役牌（發）
        Kind::ValueHonourGreenDragon => "役牌（發）",
        // 役牌（中）
        Kind::ValueHonourRedDragon => "役牌（中）",
        // 混全帯么九
        Kind::CommonEnds => {
            openned_name!("混全帯么九", has_openned, Lang::Ja)
        }
        // 純全帯么九
        Kind::PerfectEnds => {
            openned_name!("純全帯么九", has_openned, Lang::Ja)
        }
        // 混老頭
        Kind::CommonTerminals => "混老頭",
        // 小三元
        Kind::LittleDragons => "小三元",
        // 混一色
        Kind::CommonFlush => {
            openned_name!("混一色", has_openned, Lang::Ja)
        }
        // 清一色
        Kind::PerfectFlush => {
            openned_name!("清一色", has_openned, Lang::Ja)
        }
        // 国士無双
        Kind::ThirteenOrphans => "国士無双",
        // 四暗刻
        Kind::FourConcealedTriplets => "四暗刻",
        // 四暗刻単騎待ち
        Kind::FourConcealedTripletsPairWait => "四暗刻単騎待ち",
        // 大三元
        Kind::BigDragons => "大三元",
        // 小四喜
        Kind::LittleWinds => "小四喜",
        // 大四喜
        Kind::BigWinds => "大四喜",
        // 字一色
        Kind::AllHonours => "字一色",
        // 清老頭
        Kind::PerfectTerminals => "清老頭",
        // 緑一色
        Kind::AllGreen => "緑一色",
        // 九蓮宝燈
        Kind::NineGates => "九蓮宝燈",
        // 四槓子
        Kind::FourQuads => "四槓子",
        // 天和
        Kind::BlessingOfHeaven => "天和",
        // 地和
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
            (Kind::CommonFlush, "Common Flush"),
            (Kind::PerfectFlush, "Perfect Flush"),
            (Kind::ThirteenOrphans, "Thirteen Orphans"),
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
            (Kind::ThirteenOrphans, "Thirteen Orphans"),
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
            (Kind::CommonFlush, "混一色"),
            (Kind::PerfectFlush, "清一色"),
            (Kind::ThirteenOrphans, "国士無双"),
            (Kind::FourConcealedTriplets, "四暗刻"),
            (Kind::FourConcealedTripletsPairWait, "四暗刻単騎待ち"),
            (Kind::BigDragons, "大三元"),
            (Kind::LittleWinds, "小四喜"),
            (Kind::BigWinds, "大四喜"),
            (Kind::AllHonours, "字一色"),
            (Kind::PerfectTerminals, "清老頭"),
            (Kind::AllGreen, "緑一色"),
            (Kind::NineGates, "九蓮宝燈"),
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
            (Kind::ThirteenOrphans, "国士無双"),
            (Kind::FourConcealedTriplets, "四暗刻"),
            (Kind::FourConcealedTripletsPairWait, "四暗刻単騎待ち"),
            (Kind::BigDragons, "大三元"),
            (Kind::LittleWinds, "小四喜"),
            (Kind::BigWinds, "大四喜"),
            (Kind::AllHonours, "字一色"),
            (Kind::PerfectTerminals, "清老頭"),
            (Kind::AllGreen, "緑一色"),
            (Kind::NineGates, "九蓮宝燈"),
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
