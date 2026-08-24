//! Drift tests between [`crate::winning_hand::name`] and `data/yaku.json`.
//!
//! The JSON file is published for consumers outside Rust, so it has to say
//! exactly what `name.rs` says. `name.rs` is the source of truth; these tests
//! fail whenever the two are edited apart. See `data/README.md`.

use serde::Deserialize;
use strum::{EnumCount, IntoEnumIterator};

use crate::settings::Lang;
use crate::winning_hand::name::{Kind, get};

/// The published data file, embedded so the test needs no working directory.
const YAKU_JSON: &str = include_str!("../../../../data/yaku.json");

/// The JSON Schema published alongside it.
const YAKU_SCHEMA_JSON: &str = include_str!("../../../../data/yaku.schema.json");

#[derive(Debug, Deserialize)]
struct YakuData {
    schema_version: u32,
    open_suffix: OpenSuffix,
    yaku: Vec<YakuEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenSuffix {
    en: String,
    ja: String,
}

#[derive(Debug, Deserialize)]
struct YakuEntry {
    kind: String,
    en: String,
    ja: String,
    romaji: String,
    closed_only: bool,
    open_en: Option<String>,
    open_ja: Option<String>,
    value: YakuValue,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "unit", rename_all = "lowercase")]
enum YakuValue {
    Han { closed: u32, open: Option<u32> },
    Yakuman { closed: u32, open: Option<u32> },
    Mangan,
}

impl YakuValue {
    /// Whether the yaku can be scored at all on an open hand.
    fn can_be_open(&self) -> bool {
        match self {
            // Nagashi Mangan is a fixed mangan with no open/closed pair to
            // compare, and calling tiles does not disqualify it.
            YakuValue::Mangan => true,
            YakuValue::Han { open, .. } | YakuValue::Yakuman { open, .. } => open.is_some(),
        }
    }

    /// Whether the yaku is worth less on an open hand than on a closed one.
    fn drops_when_open(&self) -> bool {
        match self {
            YakuValue::Mangan => false,
            YakuValue::Han { closed, open } | YakuValue::Yakuman { closed, open } => {
                open.is_some_and(|open| open < *closed)
            }
        }
    }
}

fn load() -> YakuData {
    serde_json::from_str(YAKU_JSON).expect("data/yaku.json is not valid against the schema")
}

#[test]
fn schema_version_is_the_one_these_tests_understand() {
    // Bumping the version is a deliberate act: it means the shape changed and
    // the structs above have to change with it.
    assert_eq!(load().schema_version, 1);
}

#[test]
fn covers_every_kind_in_declaration_order() {
    let data = load();
    assert_eq!(
        data.yaku.len(),
        Kind::COUNT,
        "data/yaku.json has {} entries but Kind has {} variants",
        data.yaku.len(),
        Kind::COUNT
    );
    for (entry, kind) in data.yaku.iter().zip(Kind::iter()) {
        assert_eq!(
            entry.kind,
            format!("{kind:?}"),
            "data/yaku.json is out of order: expected {kind:?}, found {}",
            entry.kind
        );
    }
}

#[test]
fn kind_strings_are_the_serde_identifiers() {
    // Consumers key off `kind`, so it has to be the same spelling a serialized
    // `Kind` uses — not just the Debug output.
    for entry in load().yaku {
        let json = format!("\"{}\"", entry.kind);
        let decoded: Kind =
            serde_json::from_str(&json).unwrap_or_else(|_| panic!("{} is not a Kind", entry.kind));
        assert_eq!(format!("{decoded:?}"), entry.kind);
    }
}

#[test]
fn closed_names_match_name_rs() {
    for (entry, kind) in load().yaku.iter().zip(Kind::iter()) {
        assert_eq!(
            entry.en,
            get(kind, false, Lang::En),
            "English name drifted for {kind:?}"
        );
        assert_eq!(
            entry.ja,
            get(kind, false, Lang::Ja),
            "Japanese name drifted for {kind:?}"
        );
    }
}

#[test]
fn open_names_match_name_rs() {
    for (entry, kind) in load().yaku.iter().zip(Kind::iter()) {
        let expected_en = get(kind, true, Lang::En);
        let expected_ja = get(kind, true, Lang::Ja);
        assert_eq!(
            entry.open_en.as_deref().unwrap_or(&entry.en),
            expected_en,
            "open English name drifted for {kind:?}"
        );
        assert_eq!(
            entry.open_ja.as_deref().unwrap_or(&entry.ja),
            expected_ja,
            "open Japanese name drifted for {kind:?}"
        );
        // A null means "unchanged", so a spelled-out name equal to the closed
        // one would be a second way to say the same thing.
        assert_ne!(
            entry.open_en.as_deref(),
            Some(entry.en.as_str()),
            "{kind:?} should use null for an unchanged open English name"
        );
        assert_ne!(
            entry.open_ja.as_deref(),
            Some(entry.ja.as_str()),
            "{kind:?} should use null for an unchanged open Japanese name"
        );
    }
}

#[test]
fn open_names_are_the_plain_name_plus_the_suffix() {
    let data = load();
    for entry in &data.yaku {
        if let Some(open_en) = &entry.open_en {
            assert_eq!(*open_en, format!("{}{}", entry.en, data.open_suffix.en));
        }
        if let Some(open_ja) = &entry.open_ja {
            assert_eq!(*open_ja, format!("{}{}", entry.ja, data.open_suffix.ja));
        }
    }
}

#[test]
fn closed_only_agrees_with_a_missing_open_value() {
    for entry in load().yaku {
        assert_eq!(
            entry.closed_only,
            !entry.value.can_be_open(),
            "{}: closed_only and value.open disagree",
            entry.kind
        );
    }
}

#[test]
fn an_open_name_exists_exactly_when_the_value_drops() {
    for entry in load().yaku {
        assert_eq!(
            entry.open_en.is_some(),
            entry.value.drops_when_open(),
            "{}: an open name is listed for a yaku that keeps its value, or vice versa",
            entry.kind
        );
        assert_eq!(entry.open_en.is_some(), entry.open_ja.is_some());
    }
}

#[test]
fn the_schema_lists_exactly_the_kinds_that_exist() {
    // `scripts/validate-data.py` checks the data against the schema, but that
    // needs Python. This catches the half of the drift that Rust can see:
    // a variant added to or removed from `Kind` and not mirrored in the
    // schema's `kind` enum.
    let schema: serde_json::Value =
        serde_json::from_str(YAKU_SCHEMA_JSON).expect("data/yaku.schema.json is not valid JSON");
    let listed = schema["$defs"]["yaku"]["properties"]["kind"]["enum"]
        .as_array()
        .expect("the schema has no kind enum");
    let listed: Vec<&str> = listed
        .iter()
        .map(|value| value.as_str().expect("a kind enum entry is not a string"))
        .collect();
    let expected: Vec<String> = Kind::iter().map(|kind| format!("{kind:?}")).collect();
    assert_eq!(listed, expected);
}

#[test]
fn every_entry_carries_a_romaji_reading() {
    // Romaji has no counterpart in the code, so this is the only guard against
    // an entry being added with the column left blank.
    for entry in load().yaku {
        assert!(!entry.romaji.is_empty(), "{} has no romaji", entry.kind);
    }
}
