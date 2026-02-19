use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

use crate::settings::Lang;

/// 和了時の手牌の形態
#[derive(Debug, Eq, PartialEq)]
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
/// <https://en.wikipedia.org/wiki/Japanese_Mahjong_yaku>による英語名
#[derive(Debug, PartialEq, Eq, Hash, EnumCountMacro, EnumIter)]
pub enum Kind {
    /// 立直
    ReadyHand,
    /// 七対子
    SevenPairs,
    /// 流し満貫
    NagashiMangan,
    /// 門前清自摸和
    SelfPick,
    /// 一発
    OneShot,
    /// 海底撈月
    LastTileFromTheWall,
    /// 河底撈魚
    LastDiscard,
    /// 嶺上開花
    DeadWallDraw,
    /// 搶槓
    RobbingAQuad,
    /// ダブル立直
    DoubleReady,
    /// 平和
    NoPointsHand,
    /// 一盃口
    OneSetOfIdenticalSequences,
    /// 三色同順
    ThreeColorStraight,
    /// 一気通貫
    Straight,
    /// 二盃口
    TwoSetsOfIdenticalSequences,
    /// 対々和
    AllTripletHand,
    /// 三暗刻
    ThreeClosedTriplets,
    /// 三色同刻
    ThreeColorTriplets,
    /// 断么九
    AllSimples,
    /// 役牌（自風牌）
    HonorTilesPlayersWind,
    /// 役牌（場風牌）
    HonorTilesPrevailingWind,
    /// 役牌（白）
    HonorTilesWhiteDragon,
    /// 役牌（發）
    HonorTilesGreenDragon,
    /// 役牌（中）
    HonorTilesRedDragon,
    /// 混全帯么九
    TerminalOrHonorInEachSet,
    /// 純全帯么九
    TerminalInEachSet,
    /// 混老頭
    AllTerminalsAndHonors,
    /// 小三元
    LittleThreeDragons,
    /// 混一色
    HalfFlush,
    /// 清一色
    Flush,
    /// 国士無双
    ThirteenOrphans,
    /// 四暗刻
    FourConcealedTriplets,
    /// 大三元
    BigThreeDragons,
    /// 小四喜
    LittleFourWinds,
    /// 大四喜
    BigFourWinds,
    /// 字一色
    AllHonors,
    /// 清老頭
    AllTerminals,
    /// 緑一色
    AllGreen,
    /// 九蓮宝燈
    NineGates,
    /// 四槓子
    FourKans,
    /// 天和
    HeavenlyHand,
    /// 地和
    HandOfEarth,
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
/// use riichi_mahjong_rs::settings::Lang;
/// use riichi_mahjong_rs::winning_hand::name::*;
///
/// assert_eq!(get(Kind::ThreeColorStraight, true, Lang::Ja), "三色同順（鳴）");
/// assert_eq!(get(Kind::ThreeColorStraight, false, Lang::Ja), "三色同順");
/// assert_eq!(get(Kind::ThreeColorStraight, true, Lang::En), "Three Color Straight (Open)");
/// assert_eq!(get(Kind::ThreeColorStraight, false, Lang::En), "Three Color Straight");
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
    match hand_kind {
        // 立直
        Kind::ReadyHand => "Ready Hand",
        // 七対子
        Kind::SevenPairs => "Seven Pairs",
        // 流し満貫
        Kind::NagashiMangan => "Nagashi Mangan",
        // 門前清自摸和
        Kind::SelfPick => "Self Pick",
        // 一発
        Kind::OneShot => "One Shot",
        // 海底撈月
        Kind::LastTileFromTheWall => "Last Tile From The Wall",
        // 河底撈魚
        Kind::LastDiscard => "Last Discard",
        // 嶺上開花
        Kind::DeadWallDraw => "Dead Wall Draw",
        // 搶槓
        Kind::RobbingAQuad => "Robbing A Quad",
        // ダブル立直
        Kind::DoubleReady => "Double Ready",
        // 平和
        Kind::NoPointsHand => "No Points Hand",
        // 一盃口
        Kind::OneSetOfIdenticalSequences => "One Set Of Identical Sequences",
        // 三色同順
        Kind::ThreeColorStraight => {
            openned_name!("Three Color Straight", has_openned, Lang::En)
        }
        // 一気通貫
        Kind::Straight => openned_name!("Straight", has_openned, Lang::En),

        // 二盃口
        Kind::TwoSetsOfIdenticalSequences => "Two Sets Of Identical Sequences",
        // 対々和
        Kind::AllTripletHand => "All Triplet Hand",
        // 三暗刻
        Kind::ThreeClosedTriplets => "Three Closed Triplets",
        // 三色同刻
        Kind::ThreeColorTriplets => "Three Color Triplets",
        // 断么九
        Kind::AllSimples => "All Simples",
        // 役牌（自風牌）
        Kind::HonorTilesPlayersWind => "Honor Tiles (Players Wind)",
        // 役牌（場風牌）
        Kind::HonorTilesPrevailingWind => "Honor Tiles (Prevailing Wind)",
        // 役牌（白）
        Kind::HonorTilesWhiteDragon => "Honor Tiles (White Dragon)",
        // 役牌（發）
        Kind::HonorTilesGreenDragon => "Honor Tiles (Green Dragon)",
        // 役牌（中）
        Kind::HonorTilesRedDragon => "Honor Tiles (Red Dragon)",
        // 混全帯么九
        Kind::TerminalOrHonorInEachSet => {
            openned_name!("Terminal Or Honor In Each Set", has_openned, Lang::En)
        }
        // 純全帯么九
        Kind::TerminalInEachSet => {
            openned_name!("Terminal In Each Set", has_openned, Lang::En)
        }
        // 混老頭
        Kind::AllTerminalsAndHonors => "All Terminals And Honors",
        // 小三元
        Kind::LittleThreeDragons => "Little Three Dragons",
        // 混一色
        Kind::HalfFlush => {
            openned_name!("Half Flush", has_openned, Lang::En)
        }
        // 清一色
        Kind::Flush => {
            openned_name!("Flush", has_openned, Lang::En)
        }
        // 国士無双
        Kind::ThirteenOrphans => "Thirteen Orphans",
        // 四暗刻
        Kind::FourConcealedTriplets => "Four Concealed Triplets",
        // 大三元
        Kind::BigThreeDragons => "Big Three Dragons",
        // 小四喜
        Kind::LittleFourWinds => "Little Four Winds",
        // 大四喜
        Kind::BigFourWinds => "Big Four Winds",
        // 字一色
        Kind::AllHonors => "All Honors",
        // 清老頭
        Kind::AllTerminals => "All Terminals",
        // 緑一色
        Kind::AllGreen => "All Green",
        // 九蓮宝燈
        Kind::NineGates => "Nine Gates",
        // 四槓子
        Kind::FourKans => "Four Kans",
        // 天和
        Kind::HeavenlyHand => "Heavenly Hand",
        // 地和
        Kind::HandOfEarth => "Hand Of Earth",
    }
}

fn get_ja(hand_kind: Kind, has_openned: bool) -> &'static str {
    match hand_kind {
        // 立直
        Kind::ReadyHand => "立直",
        // 七対子
        Kind::SevenPairs => "七対子",
        // 流し満貫
        Kind::NagashiMangan => "流し満貫",
        // 門前清自摸和
        Kind::SelfPick => "門前清自摸和",
        // 一発
        Kind::OneShot => "一発",
        // 海底撈月
        Kind::LastTileFromTheWall => "海底撈月",
        // 河底撈魚
        Kind::LastDiscard => "河底撈魚",
        // 嶺上開花
        Kind::DeadWallDraw => "嶺上開花",
        // 搶槓
        Kind::RobbingAQuad => "搶槓",
        // ダブル立直
        Kind::DoubleReady => "ダブル立直",
        // 平和
        Kind::NoPointsHand => "平和",
        // 一盃口
        Kind::OneSetOfIdenticalSequences => "一盃口",
        // 三色同順
        Kind::ThreeColorStraight => {
            openned_name!("三色同順", has_openned, Lang::Ja)
        }
        // 一気通貫
        Kind::Straight => {
            openned_name!("一気通貫", has_openned, Lang::Ja)
        }
        // 二盃口
        Kind::TwoSetsOfIdenticalSequences => "二盃口",
        // 対々和
        Kind::AllTripletHand => "対々和",
        // 三暗刻
        Kind::ThreeClosedTriplets => "三暗刻",
        // 三色同刻
        Kind::ThreeColorTriplets => "三色同刻",
        // 断么九
        Kind::AllSimples => "断么九",
        // 役牌（自風牌）
        Kind::HonorTilesPlayersWind => "役牌（自風牌）",
        // 役牌（場風牌）
        Kind::HonorTilesPrevailingWind => "役牌（場風牌）",
        // 役牌（白）
        Kind::HonorTilesWhiteDragon => "役牌（白）",
        // 役牌（發）
        Kind::HonorTilesGreenDragon => "役牌（發）",
        // 役牌（中）
        Kind::HonorTilesRedDragon => "役牌（中）",
        // 混全帯么九
        Kind::TerminalOrHonorInEachSet => {
            openned_name!("混全帯么九", has_openned, Lang::Ja)
        }
        // 純全帯么九
        Kind::TerminalInEachSet => {
            openned_name!("純全帯么九", has_openned, Lang::Ja)
        }
        // 混老頭
        Kind::AllTerminalsAndHonors => "混老頭",
        // 小三元
        Kind::LittleThreeDragons => "小三元",
        // 混一色
        Kind::HalfFlush => {
            openned_name!("混一色", has_openned, Lang::Ja)
        }
        // 清一色
        Kind::Flush => {
            openned_name!("清一色", has_openned, Lang::Ja)
        }
        // 国士無双
        Kind::ThirteenOrphans => "国士無双",
        // 四暗刻
        Kind::FourConcealedTriplets => "四暗刻",
        // 大三元
        Kind::BigThreeDragons => "大三元",
        // 小四喜
        Kind::LittleFourWinds => "小四喜",
        // 大四喜
        Kind::BigFourWinds => "大四喜",
        // 字一色
        Kind::AllHonors => "字一色",
        // 清老頭
        Kind::AllTerminals => "清老頭",
        // 緑一色
        Kind::AllGreen => "緑一色",
        // 九蓮宝燈
        Kind::NineGates => "九蓮宝燈",
        // 四槓子
        Kind::FourKans => "四槓子",
        // 天和
        Kind::HeavenlyHand => "天和",
        // 地和
        Kind::HandOfEarth => "地和",
    }
}
