//! Tests for the mjai yaku name mapping.

use mahjong_core::scoring::score::{DoraLabel, ScoreItem};
use mahjong_core::winning_hand::name::Kind;
use rstest::rstest;
use strum::IntoEnumIterator;

use crate::yaku::{dora_name, kind_from_name, kind_name, score_item_name};

#[test]
fn every_yaku_has_a_name() {
    for kind in Kind::iter() {
        let name = kind_name(kind);
        assert!(!name.is_empty(), "{kind:?} has no mjai name");
        assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_'),
            "{kind:?} maps to {name:?}, which is not an mjai-style label"
        );
    }
}

/// Spellings taken from the reference implementation (gimite/mjai). They are
/// romanised inconsistently on purpose — `hoteiraoyui` and `haiteiraoyue` are
/// spelled as upstream spells them, not as a transliteration would suggest.
#[rstest]
#[case(Kind::Riichi, "reach")]
#[case(Kind::DoubleRiichi, "double_reach")]
#[case(Kind::Unbroken, "ippatsu")]
#[case(Kind::FullyConcealedHand, "menzenchin_tsumoho")]
#[case(Kind::SevenPairs, "chitoitsu")]
#[case(Kind::LastTileDraw, "haiteiraoyue")]
#[case(Kind::LastTileClaim, "hoteiraoyui")]
#[case(Kind::AfterAQuad, "rinshankaiho")]
#[case(Kind::RobbingAQuad, "chankan")]
#[case(Kind::Pinfu, "pinfu")]
#[case(Kind::TwinSequences, "ipeko")]
#[case(Kind::DoubleTwinSequences, "ryanpeko")]
#[case(Kind::MixedSequences, "sanshokudojun")]
#[case(Kind::MixedTriplets, "sanshokudoko")]
#[case(Kind::FullStraight, "ikkitsukan")]
#[case(Kind::AllTriplets, "toitoiho")]
#[case(Kind::ThreeConcealedTriplets, "sananko")]
#[case(Kind::AllInside, "tanyaochu")]
#[case(Kind::CommonEnds, "honchantaiyao")]
#[case(Kind::PerfectEnds, "junchantaiyao")]
#[case(Kind::CommonTerminals, "honroto")]
#[case(Kind::CommonFlush, "honiso")]
#[case(Kind::PerfectFlush, "chiniso")]
#[case(Kind::LittleDragons, "shosangen")]
#[case(Kind::BigDragons, "daisangen")]
#[case(Kind::LittleWinds, "shosushi")]
#[case(Kind::BigWinds, "daisushi")]
#[case(Kind::AllHonours, "tsuiso")]
#[case(Kind::PerfectTerminals, "chinroto")]
#[case(Kind::AllGreen, "ryuiso")]
#[case(Kind::ThreeQuads, "sankantsu")]
#[case(Kind::FourQuads, "sukantsu")]
#[case(Kind::BlessingOfHeaven, "tenho")]
#[case(Kind::BlessingOfEarth, "chiho")]
fn yaku_names_match_the_reference_spellings(#[case] kind: Kind, #[case] expected: &str) {
    assert_eq!(kind_name(kind), expected);
}

#[test]
fn the_three_dragon_triplets_share_one_name() {
    // Tenhou numbers them separately; mjai does not distinguish them, so a
    // hand with two dragon triplets legitimately reports sangenpai twice.
    for kind in [
        Kind::ValueHonourWhiteDragon,
        Kind::ValueHonourGreenDragon,
        Kind::ValueHonourRedDragon,
    ] {
        assert_eq!(kind_name(kind), "sangenpai");
    }
    // Seat and round winds stay distinct from each other and from the dragons.
    assert_eq!(kind_name(Kind::ValueHonourSeatWind), "jikaze");
    assert_eq!(kind_name(Kind::ValueHonourRoundWind), "bakaze");
}

#[rstest]
#[case(
    Kind::ThirteenOrphans,
    Kind::ThirteenOrphansThirteenWait,
    "kokushimuso"
)]
#[case(
    Kind::FourConcealedTriplets,
    Kind::FourConcealedTripletsPairWait,
    "suanko"
)]
#[case(Kind::NineGates, Kind::PureNineGates, "churenpoton")]
fn single_and_double_yakuman_forms_share_one_name(
    #[case] plain: Kind,
    #[case] enhanced: Kind,
    #[case] expected: &str,
) {
    // The distinction survives in the han count, not in the name.
    assert_eq!(kind_name(plain), expected);
    assert_eq!(kind_name(enhanced), expected);
}

#[rstest]
#[case(DoraLabel::Dora, "dora")]
#[case(DoraLabel::UraDora, "uradora")]
#[case(DoraLabel::RedDora, "akadora")]
#[case(DoraLabel::PeiDora, "nukidora")]
fn dora_labels_map_to_mjai_names(#[case] label: DoraLabel, #[case] expected: &str) {
    assert_eq!(dora_name(label), expected);
}

#[test]
fn score_items_dispatch_to_the_right_table() {
    assert_eq!(score_item_name(ScoreItem::Yaku(Kind::Pinfu)), "pinfu");
    assert_eq!(
        score_item_name(ScoreItem::Dora(DoraLabel::RedDora)),
        "akadora"
    );
}

// --- Drift against the published glossary data ---
//
// `data/yaku.json` publishes all three naming systems in one table, so the
// mjai column there has to agree with the mapping above. See `data/README.md`.

/// The published data file, embedded so the test needs no working directory.
const YAKU_JSON: &str = include_str!("../../../data/yaku.json");

#[derive(serde::Deserialize)]
struct YakuData {
    yaku: Vec<YakuEntry>,
}

#[derive(serde::Deserialize)]
struct YakuEntry {
    kind: Kind,
    mjai: String,
}

fn published_yaku() -> Vec<YakuEntry> {
    serde_json::from_str::<YakuData>(YAKU_JSON)
        .expect("data/yaku.json does not parse")
        .yaku
}

#[test]
fn published_mjai_labels_match_this_crate() {
    for entry in published_yaku() {
        assert_eq!(
            entry.mjai,
            kind_name(entry.kind),
            "data/yaku.json and yaku.rs disagree on {:?}",
            entry.kind
        );
    }
}

#[test]
fn every_published_label_decodes_to_a_yaku_carrying_it() {
    // kind_from_name is a partial inverse: a collapsed label decodes to one
    // documented representative, which must still be a yaku that the table
    // lists under that label.
    let entries = published_yaku();
    for entry in &entries {
        let decoded = kind_from_name(&entry.mjai)
            .unwrap_or_else(|| panic!("{} does not decode to any yaku", entry.mjai));
        assert_eq!(
            kind_name(decoded),
            entry.mjai,
            "{} decodes to {decoded:?}, which mjai calls something else",
            entry.mjai
        );
    }
}
