//! Round-trip and conformance tests for the mjai wire format.

use mahjong_core::tile::{Tile, Wind};
use rstest::rstest;

use crate::event::{MjaiEvent, RyukyokuReason};
use crate::tile::{MjaiTile, tile_from_str, tile_to_str, wind_from_str, wind_to_str};
use crate::{from_json, from_json_lines, to_json};

// --- tile notation -------------------------------------------------------

#[test]
fn every_tile_kind_round_trips() {
    for kind in 0..Tile::LEN as u32 {
        let tile = Tile::new(kind);
        let name = tile_to_str(tile);
        assert_eq!(
            tile_from_str(name),
            Some(tile),
            "tile kind {kind} did not round trip through {name:?}"
        );
    }
}

#[rstest]
#[case("E", Tile::Z1)]
#[case("S", Tile::Z2)]
#[case("W", Tile::Z3)]
#[case("N", Tile::Z4)]
#[case("P", Tile::Z5)]
#[case("F", Tile::Z6)]
#[case("C", Tile::Z7)]
fn honours_use_letters(#[case] name: &str, #[case] expected: u32) {
    assert_eq!(tile_from_str(name), Some(Tile::new(expected)));
    assert_eq!(tile_to_str(Tile::new(expected)), name);
}

#[rstest]
#[case("5mr", Tile::M5)]
#[case("5pr", Tile::P5)]
#[case("5sr", Tile::S5)]
fn red_fives_round_trip(#[case] name: &str, #[case] kind: u32) {
    let red = tile_from_str(name).expect("red five should parse");
    assert!(red.is_red_dora());
    assert_eq!(red.get(), kind);
    assert_eq!(tile_to_str(red), name);
}

#[test]
fn red_five_is_distinct_from_plain_five() {
    let plain = tile_from_str("5m").unwrap();
    let red = tile_from_str("5mr").unwrap();
    assert_ne!(plain, red);
    assert!(!plain.is_red_dora());
}

#[rstest]
#[case("1z", Tile::Z1)]
#[case("5z", Tile::Z5)]
#[case("7z", Tile::Z7)]
fn numeric_honours_are_accepted_but_never_emitted(#[case] name: &str, #[case] kind: u32) {
    assert_eq!(tile_from_str(name), Some(Tile::new(kind)));
    // Input is lenient for interop with Tenhou bridges; output stays canonical.
    assert_ne!(tile_to_str(Tile::new(kind)), name);
}

#[rstest]
#[case("")]
#[case("?")]
#[case("0m")]
#[case("10m")]
#[case("1x")]
#[case("8z")]
#[case("1mr")]
#[case("Er")]
#[case("e")]
fn invalid_tile_notation_is_rejected(#[case] name: &str) {
    assert_eq!(tile_from_str(name), None, "{name:?} should not parse");
}

#[test]
fn winds_round_trip() {
    for wind in [Wind::East, Wind::South, Wind::West, Wind::North] {
        assert_eq!(wind_from_str(wind_to_str(wind)), Some(wind));
    }
}

#[test]
fn hidden_tile_uses_question_mark() {
    let hidden: MjaiTile = serde_json::from_str("\"?\"").unwrap();
    assert!(hidden.is_hidden());
    assert_eq!(hidden.known(), None);
    assert_eq!(serde_json::to_string(&hidden).unwrap(), "\"?\"");
}

#[test]
fn known_tile_serialises_as_notation() {
    let known = MjaiTile::from(Tile::new_red(Tile::S5));
    assert_eq!(serde_json::to_string(&known).unwrap(), "\"5sr\"");
    assert_eq!(known.known(), Some(Tile::new_red(Tile::S5)));
}

// --- events --------------------------------------------------------------

/// Exact wire text for the events whose field order is worth pinning, so a
/// reordering of the enum shows up as a test failure rather than as a subtly
/// different stream.
#[rstest]
#[case(r#"{"type":"tsumo","actor":0,"pai":"1m"}"#)]
#[case(r#"{"type":"tsumo","actor":2,"pai":"?"}"#)]
#[case(r#"{"type":"dahai","actor":1,"pai":"5pr","tsumogiri":false}"#)]
#[case(r#"{"type":"dahai","actor":3,"pai":"C","tsumogiri":true}"#)]
#[case(r#"{"type":"chi","actor":1,"target":0,"pai":"5p","consumed":["6p","7p"]}"#)]
#[case(r#"{"type":"pon","actor":2,"target":1,"pai":"5s","consumed":["5s","5sr"]}"#)]
#[case(r#"{"type":"daiminkan","actor":3,"target":0,"pai":"E","consumed":["E","E","E"]}"#)]
#[case(r#"{"type":"ankan","actor":0,"consumed":["9m","9m","9m","9m"]}"#)]
#[case(r#"{"type":"kakan","actor":1,"pai":"P","consumed":["P","P","P"]}"#)]
#[case(r#"{"type":"dora","dora_marker":"3s"}"#)]
#[case(r#"{"type":"reach","actor":2}"#)]
#[case(r#"{"type":"reach_accepted","actor":2}"#)]
#[case(r#"{"type":"ryukyoku","reason":"kyushukyuhai","actor":1}"#)]
#[case(r#"{"type":"start_game","id":0}"#)]
#[case(r#"{"type":"end_kyoku"}"#)]
#[case(r#"{"type":"end_game"}"#)]
#[case(r#"{"type":"none"}"#)]
fn event_json_round_trips_verbatim(#[case] json: &str) {
    let event = from_json(json).expect("should decode");
    assert_eq!(to_json(&event).unwrap(), json);
}

#[test]
fn start_kyoku_decodes_all_fields() {
    let json = r#"{"type":"start_kyoku","bakaze":"S","kyoku":3,"honba":1,"kyotaku":2,
        "oya":2,"dora_marker":"1p","scores":[25000,24000,26000,25000],
        "tehais":[["?","?"],["1m","5mr"],["?","?"],["?","?"]]}"#;
    let event = from_json(json).expect("should decode");
    let MjaiEvent::StartKyoku {
        bakaze,
        kyoku,
        honba,
        kyotaku,
        oya,
        dora_marker,
        scores,
        tehais,
    } = event
    else {
        panic!("expected start_kyoku");
    };
    assert_eq!(bakaze, Wind::South);
    assert_eq!(kyoku, 3);
    assert_eq!(honba, 1);
    assert_eq!(kyotaku, 2);
    assert_eq!(oya, 2);
    assert_eq!(dora_marker, Tile::new(Tile::P1));
    assert_eq!(scores, vec![25000, 24000, 26000, 25000]);
    assert!(tehais[0].iter().all(|slot| slot.is_hidden()));
    assert_eq!(tehais[1][1].known(), Some(Tile::new_red(Tile::M5)));
}

#[test]
fn in_game_hora_omits_the_score_breakdown() {
    let json = r#"{"type":"hora","actor":0,"target":2,"pai":"3s"}"#;
    let event = from_json(json).expect("should decode");
    let MjaiEvent::Hora { fu, fan, yakus, .. } = &event else {
        panic!("expected hora");
    };
    assert!(fu.is_none() && fan.is_none() && yakus.is_none());
    // Absent fields must stay absent, not come back as nulls.
    assert_eq!(to_json(&event).unwrap(), json);
}

#[test]
fn replay_hora_keeps_the_score_breakdown() {
    let json = r#"{"type":"hora","actor":0,"target":0,"pai":"3s","fu":40,"fan":3,
        "yakus":[["riichi",1],["menzen_tsumo",1],["pinfu",1]],"hora_points":5200,
        "deltas":[5200,-1300,-2600,-1300],"scores":[30200,23700,22400,23700]}"#;
    let event = from_json(json).expect("should decode");
    let MjaiEvent::Hora {
        fu,
        fan,
        yakus,
        hora_points,
        deltas,
        ..
    } = &event
    else {
        panic!("expected hora");
    };
    assert_eq!(*fu, Some(40));
    assert_eq!(*fan, Some(3));
    assert_eq!(yakus.as_ref().unwrap().len(), 3);
    assert_eq!(*hora_points, Some(5200));
    assert_eq!(deltas.as_ref().unwrap()[0], 5200);
}

#[test]
fn replay_ryukyoku_keeps_tenpai_and_hands() {
    let json = r#"{"type":"ryukyoku","reason":"fanpai","tenpais":[true,false,false,true],"tehais":[["1m"],["?"],["?"],["9s"]],"deltas":[1500,-1500,-1500,1500],"scores":[26500,23500,23500,26500]}"#;
    let event = from_json(json).expect("should decode");
    let MjaiEvent::Ryukyoku {
        tenpais, tehais, ..
    } = &event
    else {
        panic!("expected ryukyoku");
    };
    assert_eq!(tenpais.as_ref().unwrap(), &vec![true, false, false, true]);
    assert_eq!(
        tehais.as_ref().unwrap()[3][0].known(),
        Some(Tile::new(Tile::S9))
    );
    assert_eq!(to_json(&event).unwrap(), json);
}

#[rstest]
#[case("fanpai", RyukyokuReason::Fanpai)]
#[case("kyushukyuhai", RyukyokuReason::Kyushukyuhai)]
#[case("sufonrenta", RyukyokuReason::Sufonrenta)]
#[case("suchareach", RyukyokuReason::Suchareach)]
#[case("sanchaho", RyukyokuReason::Sanchaho)]
#[case("sukaikan", RyukyokuReason::Sukaikan)]
#[case("nagashimangan", RyukyokuReason::Nagashimangan)]
fn known_ryukyoku_reasons_map_to_variants(#[case] raw: &str, #[case] expected: RyukyokuReason) {
    let json = format!(r#"{{"type":"ryukyoku","reason":"{raw}"}}"#);
    let event = from_json(&json).expect("should decode");
    let MjaiEvent::Ryukyoku { reason, .. } = &event else {
        panic!("expected ryukyoku");
    };
    assert_eq!(*reason, expected);
    assert_eq!(to_json(&event).unwrap(), json);
}

#[test]
fn unknown_ryukyoku_reason_survives_a_round_trip() {
    let json = r#"{"type":"ryukyoku","reason":"some_future_reason"}"#;
    let event = from_json(json).expect("should decode");
    let MjaiEvent::Ryukyoku { reason, .. } = &event else {
        panic!("expected ryukyoku");
    };
    assert_eq!(
        *reason,
        RyukyokuReason::Other("some_future_reason".to_owned())
    );
    assert_eq!(to_json(&event).unwrap(), json);
}

#[test]
fn unknown_event_type_is_rejected() {
    assert!(from_json(r#"{"type":"nukidora","actor":0}"#).is_err());
}

#[test]
fn invalid_tile_in_an_event_is_rejected() {
    assert!(from_json(r#"{"type":"dora","dora_marker":"1x"}"#).is_err());
}

// --- helpers -------------------------------------------------------------

#[test]
fn actor_is_reported_only_where_the_event_has_one() {
    assert_eq!(
        from_json(r#"{"type":"reach","actor":3}"#).unwrap().actor(),
        Some(3)
    );
    assert_eq!(from_json(r#"{"type":"end_game"}"#).unwrap().actor(), None);
    assert_eq!(
        from_json(r#"{"type":"dora","dora_marker":"1p"}"#)
            .unwrap()
            .actor(),
        None
    );
}

#[test]
fn only_declarable_events_count_as_player_responses() {
    let dahai = from_json(r#"{"type":"dahai","actor":0,"pai":"1m","tsumogiri":true}"#).unwrap();
    assert!(dahai.is_player_response());

    // A player is dealt tiles; it never announces the draw itself.
    let tsumo = from_json(r#"{"type":"tsumo","actor":0,"pai":"1m"}"#).unwrap();
    assert!(!tsumo.is_player_response());

    let dora = from_json(r#"{"type":"dora","dora_marker":"1p"}"#).unwrap();
    assert!(!dora.is_player_response());

    let declared_draw =
        from_json(r#"{"type":"ryukyoku","reason":"kyushukyuhai","actor":0}"#).unwrap();
    assert!(declared_draw.is_player_response());
    let announced_draw = from_json(r#"{"type":"ryukyoku","reason":"fanpai"}"#).unwrap();
    assert!(!announced_draw.is_player_response());
}

#[rstest]
#[case(r#"{"type":"chi","actor":1,"target":0,"pai":"5p","consumed":["6p"]}"#)]
#[case(r#"{"type":"pon","actor":1,"target":0,"pai":"5p","consumed":["5p","5p","5p"]}"#)]
#[case(r#"{"type":"ankan","actor":0,"consumed":["9m","9m","9m"]}"#)]
#[case(r#"{"type":"kakan","actor":0,"pai":"P","consumed":["P","P"]}"#)]
fn wrong_consumed_arity_is_caught_by_validate(#[case] json: &str) {
    let event = from_json(json).expect("decoding stays permissive");
    assert!(event.validate().is_err());
}

#[test]
fn correct_consumed_arity_validates() {
    let json = r#"{"type":"chi","actor":1,"target":0,"pai":"5p","consumed":["6p","7p"]}"#;
    assert!(from_json(json).unwrap().validate().is_ok());
}

#[rstest]
#[case(r#"{"type":"start_game","id":4,"names":["a","b","c","d"]}"#)]
#[case(r#"{"type":"dahai","actor":4,"pai":"1m","tsumogiri":true}"#)]
#[case(r#"{"type":"pon","actor":0,"target":9,"pai":"1m","consumed":["1m","1m"]}"#)]
#[case(r#"{"type":"hora","actor":0,"target":4,"pai":"1m"}"#)]
fn out_of_range_actors_are_caught_by_validate(#[case] json: &str) {
    let event = from_json(json).expect("decoding stays permissive");
    assert!(event.validate().is_err());
}

#[test]
fn a_stream_of_events_parses() {
    let log = "\
{\"type\":\"start_game\",\"id\":0,\"names\":[\"a\",\"b\",\"c\",\"d\"]}
{\"type\":\"start_kyoku\",\"bakaze\":\"E\",\"kyoku\":1,\"honba\":0,\"kyotaku\":0,\"oya\":0,\"dora_marker\":\"2m\",\"scores\":[25000,25000,25000,25000],\"tehais\":[[],[],[],[]]}
{\"type\":\"tsumo\",\"actor\":0,\"pai\":\"1m\"}

{\"type\":\"dahai\",\"actor\":0,\"pai\":\"1m\",\"tsumogiri\":true}
{\"type\":\"end_kyoku\"}
{\"type\":\"end_game\"}
";
    let events = from_json_lines(log).expect("stream should decode");
    assert_eq!(events.len(), 6);
    assert!(matches!(events[0], MjaiEvent::StartGame { .. }));
    assert!(matches!(events[5], MjaiEvent::EndGame));
}
