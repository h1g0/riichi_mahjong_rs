//! Yaku names in mjai spelling.
//!
//! mjai identifies yaku by romaji labels (`chitoitsu`, `honchantaiyao`) that
//! are a third naming system alongside this project's two: the WRC English
//! names and the Japanese names in `docs/glossary.md`. They are not derivable
//! from either, so they are listed here explicitly.
//!
//! The spellings follow the reference implementation (gimite/mjai), which
//! derives them from Tenhou's yaku ids. That has two consequences worth
//! knowing:
//!
//! * Several distinct [`Kind`]s share one mjai name. Tenhou does not
//!   distinguish the three dragon triplets, nor a plain four-concealed-triplet
//!   hand from the pair-wait form, so neither does mjai. A hand holding two
//!   different dragon triplets legitimately reports `sangenpai` twice.
//! * mjai has no open/closed variants. A yaku that loses han when open keeps
//!   the same name, and the han count carries the difference.

use mahjong_core::scoring::score::{DoraLabel, ScoreItem};
use mahjong_core::winning_hand::name::Kind;

/// Returns the mjai name for a yaku.
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Riichi => "reach",
        Kind::DoubleRiichi => "double_reach",
        Kind::Unbroken => "ippatsu",
        Kind::FullyConcealedHand => "menzenchin_tsumoho",
        Kind::SevenPairs => "chitoitsu",
        Kind::LastTileDraw => "haiteiraoyue",
        Kind::LastTileClaim => "hoteiraoyui",
        Kind::AfterAQuad => "rinshankaiho",
        Kind::RobbingAQuad => "chankan",
        Kind::Pinfu => "pinfu",
        Kind::TwinSequences => "ipeko",
        Kind::MixedSequences => "sanshokudojun",
        Kind::FullStraight => "ikkitsukan",
        Kind::DoubleTwinSequences => "ryanpeko",
        Kind::AllTriplets => "toitoiho",
        Kind::ThreeConcealedTriplets => "sananko",
        Kind::MixedTriplets => "sanshokudoko",
        Kind::AllInside => "tanyaochu",
        Kind::ValueHonourSeatWind => "jikaze",
        Kind::ValueHonourRoundWind => "bakaze",
        // Tenhou numbers the three dragons separately but mjai gives them one
        // name, so a hand with two dragon triplets reports it twice.
        Kind::ValueHonourWhiteDragon
        | Kind::ValueHonourGreenDragon
        | Kind::ValueHonourRedDragon => "sangenpai",
        Kind::CommonEnds => "honchantaiyao",
        Kind::PerfectEnds => "junchantaiyao",
        Kind::CommonTerminals => "honroto",
        Kind::LittleDragons => "shosangen",
        Kind::ThreeQuads => "sankantsu",
        Kind::CommonFlush => "honiso",
        Kind::PerfectFlush => "chiniso",
        Kind::ThirteenOrphans | Kind::ThirteenOrphansThirteenWait => "kokushimuso",
        Kind::FourConcealedTriplets | Kind::FourConcealedTripletsPairWait => "suanko",
        Kind::BigDragons => "daisangen",
        Kind::LittleWinds => "shosushi",
        Kind::BigWinds => "daisushi",
        Kind::AllHonours => "tsuiso",
        Kind::PerfectTerminals => "chinroto",
        Kind::AllGreen => "ryuiso",
        Kind::NineGates | Kind::PureNineGates => "churenpoton",
        Kind::FourQuads => "sukantsu",
        Kind::BlessingOfHeaven => "tenho",
        Kind::BlessingOfEarth => "chiho",
        // Not an mjai yaku: Tenhou ends the hand as a draw and mjai reports it
        // as `ryukyoku` with reason `nagashimangan`, which is how the encoder
        // emits it. This arm exists only so the mapping stays total.
        Kind::NagashiMangan => "nagashimangan",
    }
}

/// Returns the mjai name for a dora line of the score breakdown.
pub fn dora_name(label: DoraLabel) -> &'static str {
    match label {
        DoraLabel::Dora => "dora",
        DoraLabel::UraDora => "uradora",
        DoraLabel::RedDora => "akadora",
        // Three-player only, and not part of the four-player mjai vocabulary.
        // Named after the extraction rather than folded into `dora` so a log
        // cannot silently misreport where the han came from.
        DoraLabel::PeiDora => "nukidora",
    }
}

/// Returns the mjai name for one line of the score breakdown.
pub fn score_item_name(item: ScoreItem) -> &'static str {
    match item {
        ScoreItem::Yaku(kind) => kind_name(kind),
        ScoreItem::Dora(label) => dora_name(label),
    }
}

/// Returns the yaku an mjai name refers to, if this crate knows it.
///
/// This is a partial inverse of [`kind_name`] and cannot be a faithful one.
/// mjai collapses distinct yaku onto a single name — the three dragon triplets
/// onto `sangenpai`, the two four-concealed-triplet forms onto `suanko`, and
/// so on — so those names decode to a documented representative and the
/// original distinction is unrecoverable. Round-tripping a *name* through this
/// function and back is stable; round-tripping a [`Kind`] is not.
pub fn kind_from_name(name: &str) -> Option<Kind> {
    Some(match name {
        "reach" => Kind::Riichi,
        "double_reach" => Kind::DoubleRiichi,
        "ippatsu" => Kind::Unbroken,
        "menzenchin_tsumoho" => Kind::FullyConcealedHand,
        "chitoitsu" => Kind::SevenPairs,
        "haiteiraoyue" => Kind::LastTileDraw,
        "hoteiraoyui" => Kind::LastTileClaim,
        "rinshankaiho" => Kind::AfterAQuad,
        "chankan" => Kind::RobbingAQuad,
        "pinfu" => Kind::Pinfu,
        "ipeko" => Kind::TwinSequences,
        "sanshokudojun" => Kind::MixedSequences,
        "ikkitsukan" => Kind::FullStraight,
        "ryanpeko" => Kind::DoubleTwinSequences,
        "toitoiho" => Kind::AllTriplets,
        "sananko" => Kind::ThreeConcealedTriplets,
        "sanshokudoko" => Kind::MixedTriplets,
        "tanyaochu" => Kind::AllInside,
        "jikaze" => Kind::ValueHonourSeatWind,
        "bakaze" => Kind::ValueHonourRoundWind,
        // Representative: the white dragon stands in for all three.
        "sangenpai" => Kind::ValueHonourWhiteDragon,
        "honchantaiyao" => Kind::CommonEnds,
        "junchantaiyao" => Kind::PerfectEnds,
        "honroto" => Kind::CommonTerminals,
        "shosangen" => Kind::LittleDragons,
        "sankantsu" => Kind::ThreeQuads,
        "honiso" => Kind::CommonFlush,
        "chiniso" => Kind::PerfectFlush,
        // Representatives: the plain form stands in for the enhanced one.
        "kokushimuso" => Kind::ThirteenOrphans,
        "suanko" => Kind::FourConcealedTriplets,
        "churenpoton" => Kind::NineGates,
        "daisangen" => Kind::BigDragons,
        "shosushi" => Kind::LittleWinds,
        "daisushi" => Kind::BigWinds,
        "tsuiso" => Kind::AllHonours,
        "chinroto" => Kind::PerfectTerminals,
        "ryuiso" => Kind::AllGreen,
        "sukantsu" => Kind::FourQuads,
        "tenho" => Kind::BlessingOfHeaven,
        "chiho" => Kind::BlessingOfEarth,
        "nagashimangan" => Kind::NagashiMangan,
        _ => return None,
    })
}

/// Returns the dora label an mjai name refers to, if this crate knows it.
pub fn dora_from_name(name: &str) -> Option<DoraLabel> {
    Some(match name {
        "dora" => DoraLabel::Dora,
        "uradora" => DoraLabel::UraDora,
        "akadora" => DoraLabel::RedDora,
        "nukidora" => DoraLabel::PeiDora,
        _ => return None,
    })
}

/// Returns the score-breakdown line an mjai name refers to.
pub fn score_item_from_name(name: &str) -> Option<ScoreItem> {
    kind_from_name(name)
        .map(ScoreItem::Yaku)
        .or_else(|| dora_from_name(name).map(ScoreItem::Dora))
}
