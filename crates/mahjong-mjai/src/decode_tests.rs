//! Tests for the mjai to `ServerEvent` translation.

use mahjong_core::tile::{Tile, Wind};
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::{CallType, ClientAction, ServerEvent};
use mahjong_server::table::GameSettings;

use crate::decode::MjaiDecoder;
use crate::encode::MjaiEncoder;
use crate::event::{MjaiEvent, RyukyokuReason};
use crate::tile::MjaiTile;

fn names() -> Vec<String> {
    (0..4).map(|seat| format!("p{seat}")).collect()
}

#[test]
fn a_draw_rebuilds_reported_tenpai_and_hands() {
    let mut decoder = MjaiDecoder::new(0);
    let events = decoder.decode(&MjaiEvent::Ryukyoku {
        reason: RyukyokuReason::Fanpai,
        actor: None,
        tenpais: Some(vec![false, true, false, false]),
        tehais: Some(vec![
            vec![MjaiTile::Hidden; 13],
            vec![MjaiTile::Known(Tile::new(Tile::M1)); 13],
            vec![MjaiTile::Hidden; 13],
            vec![MjaiTile::Hidden; 13],
        ]),
        deltas: Some(vec![0, 3000, -1000, -2000]),
        scores: Some(vec![25000, 28000, 24000, 23000]),
    });
    let [
        ServerEvent::RoundDraw {
            tenpai,
            player_hands,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("expected a round draw");
    };
    assert_eq!(tenpai, &vec![Wind::South]);
    assert_eq!(player_hands.len(), 1);
    assert_eq!(player_hands[0].wind, Wind::South);
    assert_eq!(player_hands[0].hand.len(), 13);
}

/// Plays a game and returns seat 0's in-game-mode mjai log.
fn in_game_log(seed: u64) -> Vec<MjaiEvent> {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 1..4 {
        driver.set_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
    }
    driver.start_game_with_seed(seed);

    let mut encoder = MjaiEncoder::new(names());
    let mut log = Vec::new();
    for _ in 0..20_000 {
        driver.run_until_blocked();
        for event in driver.drain_events(0) {
            log.extend(encoder.encode(&event));
        }
        if driver.is_round_over() {
            if driver.is_game_over() {
                break;
            }
            driver.next_round();
        } else if !driver.force_default_action(0) {
            driver.tick();
        }
    }
    log.extend(encoder.end_game());
    log
}

/// Strips the fields that cannot survive a decode, so the round trip compares
/// only what the protocol actually round-trips.
///
/// `yakus` is dropped because mjai collapses several yaku onto one name, and
/// `fu`/`fan`/`hora_points` because the decoder cannot recover the parts of the
/// score breakdown that never reach the wire in the first place.
fn normalise(event: &MjaiEvent) -> MjaiEvent {
    let mut event = event.clone();
    match &mut event {
        MjaiEvent::Hora {
            yakus,
            fu,
            fan,
            hora_points,
            deltas,
            ..
        } => {
            *yakus = None;
            *fu = None;
            *fan = None;
            *hora_points = None;
            *deltas = None;
        }
        MjaiEvent::Ryukyoku {
            tehais: Some(tehais),
            ..
        } => {
            // ServerEvent can carry revealed tiles but has no representation
            // for the number of concealed placeholders in an unrevealed hand.
            for hand in tehais {
                hand.retain(|tile| !tile.is_hidden());
            }
        }
        _ => {}
    }
    event
}

#[test]
fn a_log_survives_a_decode_and_re_encode() {
    let original = in_game_log(42);
    assert!(original.len() > 50);

    let mut decoder = MjaiDecoder::new(0);
    let mut encoder = MjaiEncoder::new(names());
    let mut round_tripped = Vec::new();
    for event in &original {
        for server_event in decoder.decode(event) {
            round_tripped.extend(encoder.encode(&server_event));
        }
    }
    round_tripped.extend(encoder.end_game());

    let expected: Vec<MjaiEvent> = original.iter().map(normalise).collect();
    let actual: Vec<MjaiEvent> = round_tripped.iter().map(normalise).collect();

    assert_eq!(
        actual.len(),
        expected.len(),
        "event count changed across the round trip"
    );
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            actual, expected,
            "event {index} changed across the round trip"
        );
    }
}

#[test]
fn the_decoded_hand_tracks_the_real_one() {
    // The decoder rebuilds its own hand from the stream; if that drifts, every
    // decision made from it is made on a hand the player does not hold.
    let log = in_game_log(7);
    let mut decoder = MjaiDecoder::new(0);
    let mut expected_len: usize = 0;

    for event in &log {
        decoder.decode(event);
        match event {
            MjaiEvent::StartKyoku { .. } => expected_len = 13,
            MjaiEvent::Tsumo { actor: 0, .. } => expected_len += 1,
            MjaiEvent::Dahai { actor: 0, .. } => expected_len -= 1,
            MjaiEvent::Chi { actor: 0, .. } | MjaiEvent::Pon { actor: 0, .. } => {
                expected_len -= 2;
            }
            MjaiEvent::Daiminkan { actor: 0, .. } => expected_len -= 3,
            MjaiEvent::Ankan { actor: 0, .. } => expected_len -= 4,
            MjaiEvent::Kakan { actor: 0, .. } => expected_len -= 1,
            _ => continue,
        }
        let held = decoder.hand().len() + usize::from(decoder.last_drawn().is_some());
        assert_eq!(held, expected_len, "hand size drifted after {event:?}");
    }
}

#[test]
fn seat_winds_invert_the_encoders_mapping() {
    let mut decoder = MjaiDecoder::new(2);
    // East 3 is dealt by seat 2, so seat 2 is East and seat 3 is South.
    decoder.decode(&MjaiEvent::StartKyoku {
        bakaze: Wind::East,
        kyoku: 3,
        honba: 0,
        kyotaku: 0,
        oya: 2,
        dora_marker: Tile::new(Tile::P1),
        scores: vec![25000; 4],
        tehais: vec![Vec::new(); 4],
    });
    let started = decoder.decode(&MjaiEvent::Tsumo {
        actor: 3,
        pai: crate::MjaiTile::Hidden,
    });
    let ServerEvent::OtherPlayerDrew { player, .. } = &started[0] else {
        panic!("expected a draw");
    };
    assert_eq!(*player, Wind::South);
}

#[test]
fn riichi_takes_the_deposit_when_it_is_declared() {
    // The server deducts on declaration, not on acceptance, so the decoder has
    // to do the same or the scores disagree for the rest of the hand.
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku());

    let declared = decoder.decode(&MjaiEvent::Reach { actor: 0 });
    let ServerEvent::PlayerRiichi {
        scores,
        riichi_sticks,
        player,
    } = &declared[0]
    else {
        panic!("expected riichi");
    };
    assert_eq!(*player, Wind::East);
    assert_eq!(scores[0], 24000);
    assert_eq!(*riichi_sticks, 1);

    // Acceptance is bookkeeping the server does not model.
    assert!(
        decoder
            .decode(&MjaiEvent::ReachAccepted {
                actor: 0,
                deltas: None,
                scores: None,
            })
            .is_empty()
    );
}

#[test]
fn a_call_rebuilds_the_meld_the_server_would_report() {
    let mut decoder = MjaiDecoder::new(1);
    decoder.decode(&start_kyoku());
    let called = decoder.decode(&MjaiEvent::Pon {
        actor: 1,
        target: 0,
        pai: Tile::new(Tile::S5),
        consumed: vec![Tile::new(Tile::S5), Tile::new_red(Tile::S5)],
    });
    let ServerEvent::PlayerCalled {
        call_type,
        called_tile,
        tiles,
        ..
    } = &called[0]
    else {
        panic!("expected a call");
    };
    assert_eq!(*call_type, CallType::Pon);
    assert_eq!(*called_tile, Tile::new(Tile::S5));
    // The server reports the whole meld, called tile included.
    assert_eq!(tiles.len(), 3);
    assert!(tiles.iter().any(|tile| tile.is_red_dora()));
    // Our own call is followed by a hand resync.
    assert!(matches!(called[1], ServerEvent::HandUpdated { .. }));
}

#[test]
fn a_tsumogiri_action_names_the_drawn_tile() {
    // ClientAction says "discard what I just drew"; mjai has no such shorthand
    // and always needs the tile, so the decoder supplies it from the draw.
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku());
    decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: crate::MjaiTile::Known(Tile::new(Tile::M9)),
    });

    let action = decoder
        .encode_action(&ClientAction::Discard { tile: None })
        .expect("a tsumogiri should render");
    assert_eq!(
        action,
        MjaiEvent::Dahai {
            actor: 0,
            pai: Tile::new(Tile::M9),
            tsumogiri: true,
        }
    );
}

#[test]
fn a_hand_discard_names_the_chosen_tile() {
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku());
    decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: crate::MjaiTile::Known(Tile::new(Tile::M9)),
    });

    let action = decoder
        .encode_action(&ClientAction::Discard {
            tile: Some(Tile::new(Tile::P3)),
        })
        .expect("a hand discard should render");
    assert_eq!(
        action,
        MjaiEvent::Dahai {
            actor: 0,
            pai: Tile::new(Tile::P3),
            tsumogiri: false,
        }
    );
}

#[test]
fn actions_mjai_cannot_express_are_refused() {
    // Better to decline than to invent a move: these need the discarded tile
    // or the meld being formed, which the action alone does not carry.
    let decoder = MjaiDecoder::new(0);
    assert!(decoder.encode_action(&ClientAction::Ron).is_none());
    assert!(
        decoder
            .encode_action(&ClientAction::Kan { tile_index: 0 })
            .is_none()
    );
    assert!(decoder.encode_action(&ClientAction::Pei).is_none());
    assert_eq!(
        decoder.encode_action(&ClientAction::Pass),
        Some(MjaiEvent::Pass)
    );
}

#[test]
fn nine_terminals_is_derived_and_encoded_as_a_draw_declaration() {
    let hand = mahjong_core::hand::Hand::from("1119m1119p119s12z");
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(hand.tiles()));

    let events = decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: MjaiTile::Known(Tile::new(Tile::Z3)),
    });
    assert!(matches!(events[0], ServerEvent::TileDrawn { .. }));
    assert!(matches!(events[1], ServerEvent::NineTerminalsAvailable));

    assert_eq!(
        decoder.encode_action(&ClientAction::NineTerminals { declare: true }),
        Some(MjaiEvent::Ryukyoku {
            reason: RyukyokuReason::Kyushukyuhai,
            actor: Some(0),
            tenpais: None,
            tehais: None,
            deltas: None,
            scores: None,
        })
    );
    assert_eq!(
        decoder.encode_action(&ClientAction::NineTerminals { declare: false }),
        Some(MjaiEvent::Pass)
    );
}

/// Plays a game and returns seat 0's server events paired with the mjai log
/// encoded from them, so derived legality can be checked against the truth.
fn log_with_ground_truth(seed: u64) -> (Vec<ServerEvent>, Vec<MjaiEvent>) {
    let mut driver = GameDriver::new(GameSettings::default());
    // Seat 0 has to both play and buffer: driven by force_default_action it
    // would tsumogiri all game, never reach tenpai, and never exercise the
    // legality rules this test exists to check.
    for seat in 0..4 {
        driver.set_shadow_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
        driver.set_cpu_controlled(seat, true);
    }
    driver.start_game_with_seed(seed);

    let mut encoder = MjaiEncoder::new(names());
    let mut truth = Vec::new();
    let mut log = Vec::new();
    for _ in 0..20_000 {
        driver.run_until_blocked();
        for event in driver.drain_events(0) {
            log.extend(encoder.encode(&event));
            truth.push(event);
        }
        if driver.is_round_over() {
            if driver.is_game_over() {
                break;
            }
            driver.next_round();
        } else {
            driver.tick();
        }
    }
    (truth, log)
}

#[test]
fn derived_legality_never_claims_more_than_the_server_allowed() {
    // The dangerous direction is a false positive: a bot that believes an
    // illegal move is legal will send it and be rejected. Being *more*
    // conservative than the server is safe, and is expected wherever the rule
    // depends on a choice the log does not record (declining a ron).
    let (truth, log) = log_with_ground_truth(42);

    let mut decoder = MjaiDecoder::new(0);
    let mut derived = Vec::new();
    for event in &log {
        derived.extend(decoder.decode(event));
    }

    let truth_draws: Vec<_> = truth
        .iter()
        .filter_map(|event| match event {
            ServerEvent::TileDrawn {
                can_tsumo,
                can_riichi,
                ..
            } => Some((*can_tsumo, *can_riichi)),
            _ => None,
        })
        .collect();
    let derived_draws: Vec<_> = derived
        .iter()
        .filter_map(|event| match event {
            ServerEvent::TileDrawn {
                can_tsumo,
                can_riichi,
                ..
            } => Some((*can_tsumo, *can_riichi)),
            _ => None,
        })
        .collect();

    assert_eq!(truth_draws.len(), derived_draws.len(), "draw count differs");
    for (index, (derived, truth)) in derived_draws.iter().zip(truth_draws.iter()).enumerate() {
        assert!(
            !derived.0 || truth.0,
            "draw {index}: claimed a tsumo the server did not allow"
        );
        assert!(
            !derived.1 || truth.1,
            "draw {index}: claimed a riichi the server did not allow"
        );
    }
}

#[test]
fn a_ready_hand_is_offered_riichi() {
    // Whether a game happens to produce a riichi chance is luck; that the
    // wiring reaches the shared rules at all is not, so it is checked on a
    // hand built for the purpose.
    let ready = tiles(&[
        Tile::M1,
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M5,
        Tile::M6,
        Tile::M7,
        Tile::M8,
        Tile::M9,
        Tile::S1,
        Tile::S2,
        Tile::S3,
        Tile::P5,
    ]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&ready));
    let drawn = decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: crate::MjaiTile::Known(Tile::new(Tile::S9)),
    });
    let ServerEvent::TileDrawn { can_riichi, .. } = &drawn[0] else {
        panic!("expected a draw");
    };
    assert!(can_riichi, "a ready closed hand should be offered riichi");
}

#[test]
fn a_hand_far_from_ready_is_not_offered_riichi() {
    let scattered = tiles(&[
        Tile::M1,
        Tile::M3,
        Tile::M5,
        Tile::M7,
        Tile::M9,
        Tile::P1,
        Tile::P3,
        Tile::P5,
        Tile::P7,
        Tile::P9,
        Tile::S1,
        Tile::S3,
        Tile::S5,
    ]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&scattered));
    let drawn = decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: crate::MjaiTile::Known(Tile::new(Tile::S9)),
    });
    let ServerEvent::TileDrawn { can_riichi, .. } = &drawn[0] else {
        panic!("expected a draw");
    };
    assert!(!can_riichi);
}

#[test]
fn a_discard_this_seat_can_claim_is_offered() {
    let hand = tiles(&[
        Tile::P5,
        Tile::P5,
        Tile::M1,
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M5,
        Tile::M6,
        Tile::S1,
        Tile::S2,
        Tile::S3,
        Tile::S4,
        Tile::S5,
    ]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&hand));

    let discarded = decoder.decode(&MjaiEvent::Dahai {
        actor: 2,
        pai: Tile::new(Tile::P5),
        tsumogiri: false,
    });
    let offered = discarded.iter().any(
        |event| matches!(event, ServerEvent::CallAvailable { calls, .. } if !calls.is_empty()),
    );
    assert!(offered, "holding two P5, a discarded P5 should offer a pon");
}

#[test]
fn a_discard_this_seat_cannot_claim_is_not_offered() {
    let hand = tiles(&[
        Tile::M1,
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M5,
        Tile::M6,
        Tile::M7,
        Tile::M8,
        Tile::M9,
        Tile::S1,
        Tile::S2,
        Tile::S3,
        Tile::S4,
    ]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&hand));

    // Discarded by the seat across the table, so no sequence call either.
    let discarded = decoder.decode(&MjaiEvent::Dahai {
        actor: 2,
        pai: Tile::new(Tile::Z6),
        tsumogiri: false,
    });
    assert!(
        !discarded
            .iter()
            .any(|event| matches!(event, ServerEvent::CallAvailable { .. })),
        "nothing in this hand can claim a green dragon"
    );
}

#[test]
fn declining_a_ron_takes_on_furiten() {
    // Passing on a ron is a choice the log does not record, so the bot has to
    // report it or the decoder keeps offering a win the rules forbid.
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku());
    assert!(!decoder.player().is_furiten());

    decoder.declined_ron();
    assert!(decoder.player().is_furiten());

    // The furiten lifts on the next draw.
    decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: crate::MjaiTile::Known(Tile::new(Tile::M9)),
    });
    assert!(!decoder.player().is_furiten());
}

/// A hand start where this seat holds `hand`.
fn start_kyoku_with(hand: &[Tile]) -> MjaiEvent {
    let mut tehais = vec![Vec::new(); 4];
    tehais[0] = hand.iter().copied().map(crate::MjaiTile::Known).collect();
    MjaiEvent::StartKyoku {
        bakaze: Wind::East,
        kyoku: 1,
        honba: 0,
        kyotaku: 0,
        oya: 0,
        dora_marker: Tile::new(Tile::P1),
        scores: vec![25000; 4],
        tehais,
    }
}

fn tiles(kinds: &[u32]) -> Vec<Tile> {
    kinds.iter().copied().map(Tile::new).collect()
}

#[test]
fn a_concealed_quad_by_this_seat_is_applied_without_a_discarder() {
    // A concealed quad has no discarder, so asking where the tile came from
    // would trip the assertion inside meld_from_relative.
    let hand = tiles(&[
        Tile::M9,
        Tile::M9,
        Tile::M9,
        Tile::M9,
        Tile::P1,
        Tile::P2,
        Tile::P3,
        Tile::P4,
        Tile::P5,
        Tile::P6,
        Tile::S1,
        Tile::S2,
        Tile::S3,
    ]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&hand));

    let called = decoder.decode(&MjaiEvent::Ankan {
        actor: 0,
        consumed: tiles(&[Tile::M9, Tile::M9, Tile::M9, Tile::M9]),
    });
    assert!(matches!(called[0], ServerEvent::PlayerCalled { .. }));
    assert_eq!(
        decoder.player().hand.melds().len(),
        1,
        "the quad was not tracked"
    );
}

#[test]
fn a_promoted_quad_by_this_seat_is_applied() {
    // The tiles a promotion consumes sit in the existing meld rather than in
    // the hand, so a hand-contents check would wrongly reject it.
    let hand = tiles(&[
        Tile::P5,
        Tile::P5,
        Tile::P5,
        Tile::M1,
        Tile::M2,
        Tile::M3,
        Tile::M4,
        Tile::M5,
        Tile::M6,
        Tile::S1,
        Tile::S2,
        Tile::S3,
        Tile::S4,
    ]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&hand));

    decoder.decode(&MjaiEvent::Dahai {
        actor: 1,
        pai: Tile::new(Tile::P5),
        tsumogiri: false,
    });
    decoder.decode(&MjaiEvent::Pon {
        actor: 0,
        target: 1,
        pai: Tile::new(Tile::P5),
        consumed: tiles(&[Tile::P5, Tile::P5]),
    });
    assert_eq!(decoder.player().hand.melds().len(), 1);

    decoder.decode(&MjaiEvent::Dahai {
        actor: 0,
        pai: Tile::new(Tile::S4),
        tsumogiri: false,
    });
    decoder.decode(&MjaiEvent::Tsumo {
        actor: 0,
        pai: crate::MjaiTile::Known(Tile::new(Tile::P5)),
    });

    let called = decoder.decode(&MjaiEvent::Kakan {
        actor: 0,
        pai: Tile::new(Tile::P5),
        consumed: tiles(&[Tile::P5, Tile::P5, Tile::P5]),
    });
    assert!(matches!(called[0], ServerEvent::PlayerCalled { .. }));
    let melds = decoder.player().hand.melds();
    assert_eq!(melds.len(), 1);
    // A promoted quad keeps three tiles on the meld and derives the fourth,
    // so the category is what says it was promoted.
    assert!(melds[0].category.is_kan(), "the triplet was not promoted");
    assert_eq!(melds[0].expanded_tiles().len(), 4);
}

#[test]
fn a_call_this_seat_cannot_make_is_ignored_rather_than_fatal() {
    // A peer can send anything; the meld helpers panic on tiles that are not
    // held, so an impossible call must be dropped instead.
    let hand = tiles(&[Tile::M1, Tile::M2, Tile::M3]);
    let mut decoder = MjaiDecoder::new(0);
    decoder.decode(&start_kyoku_with(&hand));

    let called = decoder.decode(&MjaiEvent::Pon {
        actor: 0,
        target: 1,
        pai: Tile::new(Tile::S9),
        consumed: tiles(&[Tile::S9, Tile::S9]),
    });
    // The event still reaches the client; only the hand tracking declines it.
    assert!(matches!(called[0], ServerEvent::PlayerCalled { .. }));
    assert!(decoder.player().hand.melds().is_empty());
}

fn start_kyoku() -> MjaiEvent {
    MjaiEvent::StartKyoku {
        bakaze: Wind::East,
        kyoku: 1,
        honba: 0,
        kyotaku: 0,
        oya: 0,
        dora_marker: Tile::new(Tile::P1),
        scores: vec![25000; 4],
        tehais: vec![Vec::new(); 4],
    }
}
