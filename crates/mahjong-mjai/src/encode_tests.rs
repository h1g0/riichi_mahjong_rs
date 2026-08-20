//! Tests for the `ServerEvent` to mjai translation.

use mahjong_core::scoring::score::{DoraLabel, ScoreItem, ScoreRank};
use mahjong_core::tile::{Tile, Wind};
use mahjong_core::winning_hand::name::Kind;
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::{CallType, DrawReason, ServerEvent};
use mahjong_server::table::GameSettings;

use crate::encode::MjaiEncoder;
use crate::event::MjaiEvent;

fn encoder() -> MjaiEncoder {
    MjaiEncoder::new(vec![
        "p0".to_owned(),
        "p1".to_owned(),
        "p2".to_owned(),
        "p3".to_owned(),
    ])
}

fn game_started(seat_wind: Wind, round_number: usize, scores: [i32; 4]) -> ServerEvent {
    ServerEvent::GameStarted {
        seat_wind,
        hand: vec![Tile::new(Tile::M1); 13],
        scores,
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P1)],
        round_number,
        total_rounds: 8,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    }
}

fn discarded(player: Wind, tile: Tile) -> ServerEvent {
    ServerEvent::TileDiscarded {
        player,
        tile,
        is_tsumogiri: false,
        hand_index: None,
    }
}

fn round_won(winner: Wind, loser: Option<Wind>, scores: [i32; 4]) -> ServerEvent {
    ServerEvent::RoundWon {
        winner,
        loser,
        winning_tile: Tile::new(Tile::S3),
        scores,
        yaku_list: Vec::new(),
        han: 3,
        fu: 40,
        score_points: 5200,
        rank: ScoreRank::Normal,
        has_opened: false,
        uradora_indicators: Vec::new(),
        riichi_sticks: 0,
        honba: 0,
        honba_points: 0,
        player_hands: Vec::new(),
    }
}

// --- seat mapping --------------------------------------------------------

#[test]
fn a_players_actor_stays_fixed_as_the_seat_wind_rotates() {
    // The same player is East in the opening hand and North in the next one;
    // mjai must call them seat 0 both times.
    let mut first = encoder();
    let opening = first.encode(&game_started(Wind::East, 0, [25000; 4]));
    assert!(matches!(
        opening[0],
        MjaiEvent::StartGame { id: Some(0), .. }
    ));
    assert_eq!(first.self_actor(), Some(0));

    let mut second = encoder();
    second.encode(&game_started(Wind::North, 1, [25000; 4]));
    assert_eq!(second.self_actor(), Some(0));
}

#[test]
fn start_kyoku_reports_the_dealer_and_hand_number() {
    let mut enc = encoder();
    let events = enc.encode(&game_started(Wind::South, 2, [25000; 4]));
    let start = events
        .iter()
        .find(|e| matches!(e, MjaiEvent::StartKyoku { .. }))
        .expect("start_kyoku");
    let MjaiEvent::StartKyoku {
        kyoku, oya, tehais, ..
    } = start
    else {
        unreachable!()
    };
    // Hand index 2 is East 3, dealt by seat 2.
    assert_eq!(*kyoku, 3);
    assert_eq!(*oya, 2);
    // Only this player's hand is revealed; the rest stay concealed.
    let self_actor = enc.self_actor().unwrap();
    assert!(tehais[self_actor].iter().all(|slot| !slot.is_hidden()));
    for (seat, hand) in tehais.iter().enumerate() {
        if seat != self_actor {
            assert!(hand.iter().all(|slot| slot.is_hidden()));
        }
    }
}

#[test]
fn scores_are_passed_through_in_seat_order() {
    // ServerEvent indexes scores by seat, not by seat wind, and this project's
    // seat numbering already matches mjai's actors: seat n % 4 deals hand n.
    // Permuting here would misattribute every payment from East 2 onward.
    let mut enc = encoder();
    let events = enc.encode(&game_started(Wind::East, 1, [1000, 2000, 3000, 4000]));
    let MjaiEvent::StartKyoku { scores, .. } = events
        .iter()
        .find(|e| matches!(e, MjaiEvent::StartKyoku { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(*scores, vec![1000, 2000, 3000, 4000]);
}

/// Checks the seat-order claim against the server rather than against this
/// file's own assumption, in a hand whose dealer is not seat 0.
#[test]
fn encoded_scores_match_the_tables_own_per_seat_scores() {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 1..4 {
        driver.set_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
    }
    driver.start_game_with_seed(42);

    let mut enc = encoder();
    let mut checked = 0;
    for _ in 0..20_000 {
        driver.run_until_blocked();
        for event in driver.drain_events(0) {
            let encoded = enc.encode(&event);
            if let ServerEvent::GameStarted { round_number, .. } = &event {
                let table_scores = driver.table().scores;
                let MjaiEvent::StartKyoku { scores, oya, .. } = encoded
                    .iter()
                    .find(|e| matches!(e, MjaiEvent::StartKyoku { .. }))
                    .expect("start_kyoku")
                else {
                    unreachable!()
                };
                assert_eq!(
                    scores.as_slice(),
                    &table_scores[..],
                    "hand {round_number}: encoded scores are not the table's per-seat scores"
                );
                assert_eq!(
                    *oya,
                    round_number % 4,
                    "dealer seat and hand number disagree"
                );
                checked += 1;
            }
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
    assert!(checked >= 4, "only {checked} hands were checked");
}

// --- dora ----------------------------------------------------------------

#[test]
fn only_newly_revealed_dora_indicators_are_announced() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));

    // The opening indicator came with start_kyoku and must not repeat.
    let repeat = enc.encode(&ServerEvent::DoraIndicatorsUpdated {
        dora_indicators: vec![Tile::new(Tile::P1)],
    });
    assert!(repeat.is_empty());

    let after_kan = enc.encode(&ServerEvent::DoraIndicatorsUpdated {
        dora_indicators: vec![Tile::new(Tile::P1), Tile::new(Tile::S7)],
    });
    assert_eq!(
        after_kan,
        vec![MjaiEvent::Dora {
            dora_marker: Tile::new(Tile::S7)
        }]
    );
}

// --- riichi bracketing ---------------------------------------------------

#[test]
fn riichi_brackets_the_declaring_discard() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));

    let reach = enc.encode(&ServerEvent::PlayerRiichi {
        player: Wind::East,
        scores: [24000, 25000, 25000, 25000],
        riichi_sticks: 1,
    });
    assert_eq!(reach, vec![MjaiEvent::Reach { actor: 0 }]);

    // The declaring discard comes next, still without acceptance.
    let discard = enc.encode(&discarded(Wind::East, Tile::new(Tile::M9)));
    assert_eq!(discard.len(), 1);
    assert!(matches!(discard[0], MjaiEvent::Dahai { actor: 0, .. }));

    // Acceptance is released once the discard is known to have survived.
    let next = enc.encode(&ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 60,
    });
    assert_eq!(next[0], MjaiEvent::ReachAccepted { actor: 0 });
}

#[test]
fn a_ronned_riichi_discard_is_never_accepted() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));
    enc.encode(&ServerEvent::PlayerRiichi {
        player: Wind::East,
        scores: [24000, 25000, 25000, 25000],
        riichi_sticks: 1,
    });
    enc.encode(&discarded(Wind::East, Tile::new(Tile::S3)));

    // South rons the declaring discard, so the declaration never stood.
    let won = enc.encode(&round_won(
        Wind::South,
        Some(Wind::East),
        [18800, 31200, 25000, 25000],
    ));
    assert!(
        !won.iter()
            .any(|e| matches!(e, MjaiEvent::ReachAccepted { .. })),
        "reach_accepted must be suppressed when the declaring discard is ronned"
    );
    assert!(matches!(won[0], MjaiEvent::Hora { .. }));
}

// --- calls ---------------------------------------------------------------

#[test]
fn a_call_names_the_actual_discarder_not_the_left_neighbour() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));
    // West discards; East pons across the table.
    enc.encode(&discarded(Wind::West, Tile::new(Tile::S5)));

    let called = enc.encode(&ServerEvent::PlayerCalled {
        player: Wind::East,
        call_type: CallType::Pon,
        called_tile: Tile::new(Tile::S5),
        tiles: vec![
            Tile::new(Tile::S5),
            Tile::new_red(Tile::S5),
            Tile::new(Tile::S5),
        ],
    });
    let MjaiEvent::Pon {
        actor,
        target,
        consumed,
        ..
    } = &called[0]
    else {
        panic!("expected pon");
    };
    assert_eq!(*actor, 0);
    assert_eq!(*target, 2, "target must be West, the seat that discarded");
    // The called tile is dropped once, and the red five is not the one taken.
    assert_eq!(consumed.len(), 2);
    assert!(consumed.iter().any(|tile| tile.is_red_dora()));
}

#[test]
fn a_concealed_quad_consumes_all_four_tiles() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));
    let called = enc.encode(&ServerEvent::PlayerCalled {
        player: Wind::East,
        call_type: CallType::Ankan,
        called_tile: Tile::new(Tile::M9),
        tiles: vec![Tile::new(Tile::M9); 4],
    });
    let MjaiEvent::Ankan { consumed, .. } = &called[0] else {
        panic!("expected ankan");
    };
    assert_eq!(consumed.len(), 4);
    assert!(called[0].validate().is_ok());
}

// --- results -------------------------------------------------------------

#[test]
fn a_win_reports_score_changes_against_the_previous_scores() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));
    let won = enc.encode(&round_won(
        Wind::East,
        Some(Wind::South),
        [30200, 19800, 25000, 25000],
    ));
    let MjaiEvent::Hora {
        actor,
        target,
        deltas,
        fu,
        fan,
        ..
    } = &won[0]
    else {
        panic!("expected hora");
    };
    assert_eq!(*actor, 0);
    assert_eq!(*target, 1);
    assert_eq!(*fu, Some(40));
    assert_eq!(*fan, Some(3));
    assert_eq!(deltas.as_ref().unwrap(), &vec![5200, -5200, 0, 0]);
    assert_eq!(won[1], MjaiEvent::EndKyoku);
}

#[test]
fn a_win_reports_yaku_under_their_mjai_names() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));

    let mut event = round_won(Wind::East, None, [28000, 24000, 24000, 24000]);
    if let ServerEvent::RoundWon { yaku_list, .. } = &mut event {
        *yaku_list = vec![
            (ScoreItem::Yaku(Kind::Riichi), 1),
            (ScoreItem::Yaku(Kind::FullyConcealedHand), 1),
            (ScoreItem::Yaku(Kind::Pinfu), 1),
            (ScoreItem::Dora(DoraLabel::RedDora), 1),
        ];
    }

    let won = enc.encode(&event);
    let MjaiEvent::Hora { yakus, .. } = &won[0] else {
        panic!("expected hora");
    };
    assert_eq!(
        yakus.as_ref().unwrap(),
        &vec![
            ("reach".to_owned(), 1),
            ("menzenchin_tsumoho".to_owned(), 1),
            ("pinfu".to_owned(), 1),
            ("akadora".to_owned(), 1),
        ]
    );
}

#[test]
fn a_double_ron_reports_both_wins_but_ends_the_hand_once() {
    // The server sends one RoundWon per winner. mjai wants both hora events,
    // but a hand ends exactly once; emitting a second end_kyoku would leave
    // the log with more hand endings than hand starts.
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));

    let first = enc.encode(&round_won(
        Wind::South,
        Some(Wind::East),
        [20000, 30000, 25000, 25000],
    ));
    let second = enc.encode(&round_won(
        Wind::West,
        Some(Wind::East),
        [17000, 30000, 28000, 25000],
    ));

    assert!(matches!(first[0], MjaiEvent::Hora { .. }));
    assert_eq!(first[1], MjaiEvent::EndKyoku);
    assert!(matches!(second[0], MjaiEvent::Hora { .. }));
    assert_eq!(
        second.len(),
        1,
        "the second win must not close the hand again: {second:?}"
    );

    // A new hand re-arms the guard.
    let next = enc.encode(&game_started(Wind::East, 1, [17000, 30000, 28000, 25000]));
    assert!(
        next.iter()
            .any(|e| matches!(e, MjaiEvent::StartKyoku { .. }))
    );
    let drawn = enc.encode(&ServerEvent::RoundDraw {
        scores: [17000, 30000, 28000, 25000],
        reason: DrawReason::Exhaustive,
        tenpai: Vec::new(),
        riichi_sticks: 0,
        player_hands: Vec::new(),
        declarer: None,
    });
    assert_eq!(drawn[1], MjaiEvent::EndKyoku);
}

#[test]
fn a_self_draw_win_targets_the_winner() {
    let mut enc = encoder();
    enc.encode(&game_started(Wind::East, 0, [25000; 4]));
    let won = enc.encode(&round_won(Wind::East, None, [28000, 24000, 24000, 24000]));
    let MjaiEvent::Hora { actor, target, .. } = &won[0] else {
        panic!("expected hora");
    };
    assert_eq!(actor, target);
}

#[test]
fn draw_reasons_map_to_the_reference_spellings() {
    for (reason, expected) in [
        (DrawReason::Exhaustive, "fanpai"),
        (DrawReason::NineTerminals, "kyushukyuhai"),
        (DrawReason::FourWinds, "sufonrenta"),
        (DrawReason::FourRiichi, "suchareach"),
        (DrawReason::FourKans, "sukaikan"),
        (DrawReason::TripleRon, "sanchaho"),
    ] {
        let mut enc = encoder();
        enc.encode(&game_started(Wind::East, 0, [25000; 4]));
        let drawn = enc.encode(&ServerEvent::RoundDraw {
            scores: [25000; 4],
            reason,
            tenpai: Vec::new(),
            riichi_sticks: 0,
            player_hands: Vec::new(),
            declarer: None,
        });
        let MjaiEvent::Ryukyoku { reason, .. } = &drawn[0] else {
            panic!("expected ryukyoku");
        };
        assert_eq!(reason.as_str(), expected);
    }
}

// --- end to end ----------------------------------------------------------

/// Encodes a real game driven to completion and checks the log is coherent.
///
/// Hand-built events cannot catch ordering mistakes, because they encode the
/// order this file assumes rather than the order the server actually emits.
#[test]
fn a_real_game_encodes_into_a_well_formed_log() {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 1..4 {
        driver.set_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
    }
    driver.start_game_with_seed(42);

    let mut enc = encoder();
    let mut log = Vec::new();
    for _ in 0..20_000 {
        driver.run_until_blocked();
        for event in driver.drain_events(0) {
            log.extend(enc.encode(&event));
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
    log.extend(enc.end_game());

    assert!(
        log.len() > 50,
        "expected a substantial log, got {}",
        log.len()
    );

    // Exactly one start_game, and it opens the log.
    let start_games = log
        .iter()
        .filter(|e| matches!(e, MjaiEvent::StartGame { .. }))
        .count();
    assert_eq!(start_games, 1);
    assert!(matches!(log[0], MjaiEvent::StartGame { .. }));

    // Every call carries a legal number of consumed tiles.
    for event in &log {
        assert!(event.validate().is_ok(), "malformed event: {event:?}");
    }

    // Every hand that starts also ends, and no event escapes a hand.
    let mut in_hand = false;
    for event in &log {
        match event {
            MjaiEvent::StartKyoku { .. } => {
                assert!(!in_hand, "start_kyoku inside an unfinished hand");
                in_hand = true;
            }
            MjaiEvent::EndKyoku => {
                assert!(in_hand, "end_kyoku outside a hand");
                in_hand = false;
            }
            MjaiEvent::Dahai { .. } | MjaiEvent::Tsumo { .. } => {
                assert!(in_hand, "play event outside a hand: {event:?}");
            }
            _ => {}
        }
    }

    // A riichi is always accepted or explained by a win on that discard.
    for (index, event) in log.iter().enumerate() {
        let MjaiEvent::Reach { actor } = event else {
            continue;
        };
        let resolved = log[index + 1..].iter().find_map(|later| match later {
            MjaiEvent::ReachAccepted { actor: accepted } if accepted == actor => Some(true),
            MjaiEvent::Hora { .. } => Some(false),
            _ => None,
        });
        assert!(resolved.is_some(), "reach by {actor} was never resolved");
    }

    // Every win names at least one yaku; a hand cannot win without one.
    for event in &log {
        if let MjaiEvent::Hora { yakus, .. } = event {
            let yakus = yakus.as_ref().expect("a win must report its yaku");
            assert!(!yakus.is_empty(), "win with an empty yaku list: {event:?}");
        }
    }

    // The whole log survives a JSON round trip.
    for event in &log {
        let json = crate::to_json(event).expect("should serialise");
        assert_eq!(&crate::from_json(&json).expect("should parse"), event);
    }
}
