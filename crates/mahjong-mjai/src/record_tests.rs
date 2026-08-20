//! Tests for replay-mode recording.

use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::driver::GameDriver;
use mahjong_server::table::GameSettings;

use crate::event::MjaiEvent;
use crate::record::{MjaiRecorder, to_json_lines};

/// Runs a full game with every seat both playing and buffering, and records it.
fn record_a_game(seed: u64) -> Vec<MjaiEvent> {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 0..4 {
        // A plain CPU seat never buffers. A shadow CPU buffers but does not
        // play, so hand it control to get a seat that does both.
        driver.set_shadow_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
        driver.set_cpu_controlled(seat, true);
    }
    driver.start_game_with_seed(seed);

    let mut recorder = MjaiRecorder::new((0..4).map(|seat| format!("p{seat}")).collect::<Vec<_>>());
    for _ in 0..20_000 {
        driver.run_until_blocked();
        for seat in 0..4 {
            let events = driver.drain_events(seat);
            recorder.record(seat, events);
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
    recorder.finish()
}

#[test]
fn a_recorded_game_conceals_nothing() {
    let log = record_a_game(42);
    assert!(
        log.len() > 50,
        "expected a substantial log, got {}",
        log.len()
    );

    for event in &log {
        match event {
            MjaiEvent::Tsumo { pai, actor } => {
                assert!(
                    !pai.is_hidden(),
                    "replay mode must reveal every draw, but seat {actor} drew a hidden tile"
                );
            }
            MjaiEvent::StartKyoku { tehais, .. } => {
                for (seat, hand) in tehais.iter().enumerate() {
                    assert_eq!(hand.len(), 13, "seat {seat} has a malformed starting hand");
                    assert!(
                        hand.iter().all(|slot| !slot.is_hidden()),
                        "replay mode must reveal every starting hand, but seat {seat} is hidden"
                    );
                }
            }
            _ => {}
        }
    }
}

#[test]
fn a_recorded_game_is_structurally_sound() {
    let log = record_a_game(7);

    // Exactly one start_game, opening the log, and one end_game closing it.
    assert!(matches!(log[0], MjaiEvent::StartGame { .. }));
    assert_eq!(
        log.iter()
            .filter(|e| matches!(e, MjaiEvent::StartGame { .. }))
            .count(),
        1
    );
    assert_eq!(log.last(), Some(&MjaiEvent::EndGame));

    // Every hand opens and closes exactly once.
    let starts = log
        .iter()
        .filter(|e| matches!(e, MjaiEvent::StartKyoku { .. }))
        .count();
    let ends = log
        .iter()
        .filter(|e| matches!(e, MjaiEvent::EndKyoku))
        .count();
    assert_eq!(starts, ends, "every hand must end");
    assert!(starts >= 4, "expected several hands, got {starts}");

    for event in &log {
        assert!(event.validate().is_ok(), "malformed event: {event:?}");
    }
}

#[test]
fn a_replay_names_no_recipient_seat() {
    // In-game mode tells the receiving player which seat it holds. A replay is
    // nobody's point of view, so claiming a seat would misdescribe the log.
    let log = record_a_game(42);
    let MjaiEvent::StartGame { id, names } = &log[0] else {
        panic!("expected start_game");
    };
    assert_eq!(*id, None);
    assert_eq!(names.len(), 4);
}

#[test]
fn a_win_reveals_the_winning_hand() {
    let log = record_a_game(42);
    let wins: Vec<_> = log
        .iter()
        .filter_map(|event| match event {
            MjaiEvent::Hora { hora_tehais, .. } => Some(hora_tehais),
            _ => None,
        })
        .collect();
    assert!(!wins.is_empty(), "expected at least one win in the game");
    for hand in wins {
        let hand = hand.as_ref().expect("a recorded win reveals the hand");
        assert!(!hand.is_empty());
    }
}

#[test]
fn every_dahai_follows_a_draw_or_a_call_by_the_same_seat() {
    // A revealed log is only useful if a reader can reconstruct each hand, so
    // no discard may appear without the tile that made it possible.
    let log = record_a_game(7);
    let mut may_discard = [false; 4];
    for event in &log {
        match event {
            MjaiEvent::StartKyoku { oya, .. } => {
                may_discard = [false; 4];
                may_discard[*oya] = true;
            }
            MjaiEvent::Tsumo { actor, .. }
            | MjaiEvent::Chi { actor, .. }
            | MjaiEvent::Pon { actor, .. } => {
                may_discard[*actor] = true;
            }
            MjaiEvent::Dahai { actor, .. } => {
                assert!(
                    may_discard[*actor],
                    "seat {actor} discarded without drawing or calling"
                );
                may_discard[*actor] = false;
            }
            _ => {}
        }
    }
}

#[test]
fn the_log_serialises_to_newline_delimited_json() {
    let log = record_a_game(42);
    let text = to_json_lines(&log).expect("should serialise");

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), log.len());
    assert!(text.ends_with('\n'));
    // No line may contain a raw newline, or the framing breaks.
    assert!(lines.iter().all(|line| line.starts_with('{')));

    let parsed = crate::from_json_lines(&text).expect("should parse back");
    assert_eq!(parsed, log);
}

#[test]
fn an_empty_recording_produces_an_empty_log() {
    let recorder = MjaiRecorder::new(vec!["a".to_owned()]);
    assert!(recorder.is_empty());
    assert!(recorder.finish().is_empty());
}
