//! Tests for importing and replaying a game log.

use mahjong_core::settings::Settings;
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::ServerEvent;
use mahjong_server::table::GameSettings;

use crate::encode::MjaiEncoder;
use crate::event::MjaiEvent;
use crate::record::MjaiRecorder;
use crate::replay::{FindingKind, LogSource, MjaiReplay, audit_log};

/// Plays one whole game and records it as a fully revealed log.
///
/// The wall is derived from `seed` for every hand, not just the first, so the
/// log is reproducible (#377).
fn recorded_game(seed: u64) -> Vec<MjaiEvent> {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 0..4 {
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

/// A log this project produced must audit clean when read back: the same
/// engine scored it, so any finding is a fault in the import path rather than
/// a rules disagreement.
#[test]
fn a_game_this_engine_played_replays_without_findings() {
    for seed in [42, 7, 2024, 1] {
        let log = recorded_game(seed);
        let report = audit_log(&log, Settings::default());
        assert!(
            report.is_clean(),
            "seed {seed} produced findings: {:?}",
            report.findings
        );
        assert!(report.hands > 0, "seed {seed} produced no hands");
        assert_eq!(report.hands_skipped, 0, "seed {seed} skipped a hand");
    }
}

/// The audit is only worth anything if it compares enough games to be likely
/// to catch something, so make sure a batch of seeds actually reaches the
/// scoring and readiness comparisons.
#[test]
fn a_batch_of_games_reaches_both_comparisons() {
    let mut wins_checked = 0;
    let mut draws_checked = 0;
    for seed in 0..12u64 {
        let report = audit_log(&recorded_game(seed), Settings::default());
        assert!(
            report.is_clean(),
            "seed {seed} produced findings: {:?}",
            report.findings
        );
        wins_checked += report.wins_checked;
        draws_checked += report.draws_checked;
    }
    assert!(wins_checked >= 10, "only {wins_checked} wins were checked");
    assert!(draws_checked >= 1, "no exhaustive draw was checked");
}

/// The winner's score in a log this project produced is compared, not merely
/// counted: a log claiming a different han count must be reported.
#[test]
fn a_log_that_overstates_han_is_caught() {
    let mut log = recorded_game(42);
    let hora = log
        .iter_mut()
        .find(|event| matches!(event, MjaiEvent::Hora { .. }))
        .expect("the game should contain a win");
    let MjaiEvent::Hora { fan, .. } = hora else {
        unreachable!("filtered above");
    };
    let claimed = fan.expect("a recorded win states its han") + 1;
    *fan = Some(claimed);

    let report = audit_log(&log, Settings::default());
    let found = report.findings.iter().any(
        |finding| matches!(finding.kind, FindingKind::Han { logged, .. } if logged == claimed),
    );
    assert!(
        found,
        "an inflated han count went unreported: {:?}",
        report.findings
    );
}

/// The readiness comparison is what cross-checks shanten against real games,
/// so it too must actually fire.
#[test]
fn a_log_that_misreports_readiness_is_caught() {
    // Not every seed reaches an exhaustive draw, and which ones do depends on
    // how the CPU plays; take the first that does rather than pinning a seed
    // that a heuristics change could quietly invalidate.
    let mut logged = None;
    let mut log = Vec::new();
    for seed in 0..12u64 {
        log = recorded_game(seed);
        logged = log.iter_mut().find_map(|event| match event {
            MjaiEvent::Ryukyoku {
                tenpais: Some(tenpais),
                reason: crate::RyukyokuReason::Fanpai,
                ..
            } => {
                tenpais[0] = !tenpais[0];
                Some(tenpais[0])
            }
            _ => None,
        });
        if logged.is_some() {
            break;
        }
    }
    let Some(logged) = logged else {
        panic!("no seed reached an exhaustive draw");
    };

    let report = audit_log(&log, Settings::default());
    assert!(
        report.findings.iter().any(|finding| matches!(
            finding.kind,
            FindingKind::Ready { actor: 0, logged: claimed, .. } if claimed == logged
        )),
        "a flipped ready flag went unreported: {:?}",
        report.findings
    );
}

/// A log addressed to one seat hides the other three, so there is nothing to
/// audit. That must be reported as skipped rather than silently passing as
/// agreement.
#[test]
fn an_in_game_log_is_skipped_rather_than_checked() {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 0..4 {
        driver.set_shadow_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
        driver.set_cpu_controlled(seat, true);
    }
    driver.start_game_with_seed(42);

    // One seat's point of view, which is what an mjai player receives.
    let mut encoder = MjaiEncoder::new(vec!["p0".to_owned()]);
    let mut log = Vec::new();
    for _ in 0..20_000 {
        driver.run_until_blocked();
        for seat in 0..4 {
            let events = driver.drain_events(seat);
            if seat == 0 {
                for event in &events {
                    log.extend(encoder.encode(event));
                }
            }
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

    let report = audit_log(&log, Settings::default());
    assert!(report.hands > 0);
    assert_eq!(report.hands_skipped, report.hands);
    assert_eq!(report.wins_checked, 0);
    assert!(report.is_clean(), "{:?}", report.findings);
}

/// Feeding the log gives every seat the stream it would have received live,
/// which is what makes the imported game playable back through the engine.
#[test]
fn feeding_a_log_produces_a_stream_for_every_seat() {
    let log = recorded_game(42);
    let mut replay = MjaiReplay::new();
    let mut per_seat = [0usize; 4];
    let mut hands_started = 0;

    for event in &log {
        let events = replay.feed(event);
        assert_eq!(events.len(), 4);
        for (seat, stream) in events.iter().enumerate() {
            per_seat[seat] += stream.len();
            if matches!(event, MjaiEvent::StartKyoku { .. }) {
                hands_started += 1;
                let Some(ServerEvent::GameStarted { hand, .. }) = stream.first() else {
                    panic!("a hand must open with GameStarted for seat {seat}");
                };
                assert_eq!(hand.len(), 13, "seat {seat} was dealt a malformed hand");
            }
        }
    }

    assert!(hands_started >= 4);
    for (seat, count) in per_seat.iter().enumerate() {
        assert!(*count > 50, "seat {seat} received only {count} events");
    }
}

/// Every seat is rebuilt, not only the winner: the readiness comparison at a
/// draw depends on holding all four hands.
#[test]
fn every_seat_is_rebuilt_from_the_log() {
    let log = recorded_game(42);
    let mut replay = MjaiReplay::new();

    for event in &log {
        replay.feed(event);
        let MjaiEvent::StartKyoku { tehais, .. } = event else {
            continue;
        };
        for (seat, dealt) in tehais.iter().enumerate() {
            let mut expected: Vec<_> = dealt.iter().filter_map(|slot| slot.known()).collect();
            expected.sort();
            let mut rebuilt = replay
                .player(seat)
                .expect("seat exists")
                .hand
                .tiles()
                .to_vec();
            rebuilt.sort();
            assert_eq!(rebuilt, expected, "seat {seat} was dealt a different hand");
        }
    }
}

/// A hand-written log, so the numbers below are fixed by the rules rather than
/// by whatever this project's CPU happened to do: a full go-around of discards,
/// then a dealer riichi that wins on the very next draw.
///
/// The opening go-around matters. Declaring on the first draw would be a
/// double riichi and score two han instead of one, which is not the hand this
/// fixture is about.
///
/// Riichi + Ippatsu + Self-draw + Pinfu is 4 han 20 fu, which pays the dealer
/// 2,600 from each opponent, plus the 1,000 deposit back.
fn ippatsu_log(with_call: bool) -> Vec<MjaiEvent> {
    let mut lines = vec![
        r#"{"type":"start_game","names":["a","b","c","d"]}"#.to_owned(),
        concat!(
            r#"{"type":"start_kyoku","bakaze":"E","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"#,
            r#""dora_marker":"1z","scores":[25000,25000,25000,25000],"tehais":["#,
            r#"["2m","3m","4m","5p","6p","7p","2s","3s","4s","7s","8s","9s","9s"],"#,
            r#"["1m","1m","1m","2m","2m","2m","3m","3m","3m","4m","4m","4m","5m"],"#,
            r#"["6m","6m","6m","7m","7m","7m","8m","8m","8m","9m","9m","4p","4p"],"#,
            r#"["9m","9m","1p","1p","1p","1p","2p","2p","2p","2p","3p","3p","3p"]]}"#
        )
        .to_owned(),
    ];
    for actor in 0..4 {
        lines.push(format!(r#"{{"type":"tsumo","actor":{actor},"pai":"5z"}}"#));
        lines.push(format!(
            r#"{{"type":"dahai","actor":{actor},"pai":"5z","tsumogiri":true}}"#
        ));
    }
    lines.extend([
        r#"{"type":"tsumo","actor":0,"pai":"1z"}"#.to_owned(),
        r#"{"type":"reach","actor":0}"#.to_owned(),
        r#"{"type":"dahai","actor":0,"pai":"1z","tsumogiri":true}"#.to_owned(),
        r#"{"type":"reach_accepted","actor":0,"deltas":[-1000,0,0,0],"scores":[24000,25000,25000,25000]}"#.to_owned(),
        r#"{"type":"tsumo","actor":1,"pai":"4p"}"#.to_owned(),
        r#"{"type":"dahai","actor":1,"pai":"4p","tsumogiri":true}"#.to_owned(),
    ]);

    if with_call {
        // A call takes ippatsu off the table for everyone, including a
        // declarer who was not involved in it.
        lines.push(
            r#"{"type":"pon","actor":2,"target":1,"pai":"4p","consumed":["4p","4p"]}"#.to_owned(),
        );
        lines.push(r#"{"type":"dahai","actor":2,"pai":"9m","tsumogiri":false}"#.to_owned());
    } else {
        lines.push(r#"{"type":"tsumo","actor":2,"pai":"6z"}"#.to_owned());
        lines.push(r#"{"type":"dahai","actor":2,"pai":"6z","tsumogiri":true}"#.to_owned());
    }
    lines.push(r#"{"type":"tsumo","actor":3,"pai":"6z"}"#.to_owned());
    lines.push(r#"{"type":"dahai","actor":3,"pai":"6z","tsumogiri":true}"#.to_owned());
    lines.push(r#"{"type":"tsumo","actor":0,"pai":"6s"}"#.to_owned());

    crate::from_json_lines(&lines.join(
        "
",
    ))
    .expect("the fixture should parse")
}

/// The declaring discard must keep ippatsu alive: it is cleared by the *next*
/// discard, not by the one that announced the riichi.
#[test]
fn ippatsu_survives_the_declaring_discard() {
    let mut replay = MjaiReplay::new();
    replay.run(&ippatsu_log(false));

    let declarer = replay.player(0).expect("seat 0 exists");
    assert!(declarer.is_riichi);
    assert!(
        declarer.is_ippatsu,
        "the riichi window was closed by the declaring discard itself"
    );
}

/// Any call cancels ippatsu for every seat, not only for the caller.
#[test]
fn a_call_cancels_ippatsu_for_the_declarer() {
    let mut replay = MjaiReplay::new();
    replay.run(&ippatsu_log(true));

    let declarer = replay.player(0).expect("seat 0 exists");
    assert!(declarer.is_riichi);
    assert!(
        !declarer.is_ippatsu,
        "a call between the declaration and the win left ippatsu standing"
    );
}

/// The whole audit over a log whose numbers were worked out by hand, so a
/// change in this project's scoring shows up here as a finding rather than as
/// two matching mistakes.
#[test]
fn a_hand_written_win_audits_against_its_stated_score() {
    let mut log = ippatsu_log(false);
    log.push(
        crate::from_json(concat!(
            r#"{"type":"hora","actor":0,"target":0,"pai":"6s","fu":20,"fan":4,"#,
            r#""yakus":[["reach",1],["ippatsu",1],["menzenchin_tsumoho",1],["pinfu",1]],"#,
            r#""hora_tehais":["2m","3m","4m","5p","6p","7p","2s","3s","4s","7s","8s","9s","9s"],"#,
            r#""uradora_markers":["3z"],"hora_points":8800,"#,
            r#""deltas":[8800,-2600,-2600,-2600],"scores":[32800,22400,22400,22400]}"#
        ))
        .expect("the fixture should parse"),
    );
    log.push(MjaiEvent::EndKyoku);
    log.push(MjaiEvent::EndGame);

    let report = audit_log(&log, Settings::default());
    assert_eq!(report.hands, 1);
    assert_eq!(report.hands_skipped, 0);
    assert_eq!(report.wins, 1);
    assert_eq!(report.wins_checked, 1);
    assert!(report.is_clean(), "{:?}", report.findings);
}

/// The point transfers are compared, not just the hand's value, so a log that
/// pays the wrong amount for a correct hand is still caught.
#[test]
fn a_log_that_pays_the_wrong_amount_is_caught() {
    let mut log = ippatsu_log(false);
    log.push(
        crate::from_json(concat!(
            r#"{"type":"hora","actor":0,"target":0,"pai":"6s","fu":20,"fan":4,"#,
            r#""hora_tehais":["2m","3m","4m","5p","6p","7p","2s","3s","4s","7s","8s","9s","9s"],"#,
            r#""uradora_markers":["3z"],"hora_points":8800,"#,
            r#""deltas":[7800,-2600,-2600,-2600],"scores":[31800,22400,22400,22400]}"#
        ))
        .expect("the fixture should parse"),
    );

    let report = audit_log(&log, Settings::default());
    assert!(
        report
            .findings
            .iter()
            .any(|finding| matches!(finding.kind, FindingKind::Deltas { .. })),
        "a short payment went unreported: {:?}",
        report.findings
    );
}

/// A ron on the declaring discard: the riichi never stood, so mjai emits no
/// `reach_accepted` and the log therefore never shows the deposit being placed.
/// The round took it anyway and pays it to the winner, so the win itself has to
/// account for it or the audit reports a payment that is off by exactly 1,000.
#[test]
fn a_ron_on_the_declaring_discard_accounts_for_the_deposit() {
    let mut lines = vec![
        r#"{"type":"start_game","names":["a","b","c","d"]}"#.to_owned(),
        concat!(
            r#"{"type":"start_kyoku","bakaze":"E","kyoku":1,"honba":0,"kyotaku":0,"oya":0,"#,
            r#""dora_marker":"1z","scores":[25000,25000,25000,25000],"tehais":["#,
            r#"["1m","1m","1m","9m","9m","9m","1s","1s","1s","9s","9s","9s","7p"],"#,
            r#"["2m","3m","4m","5m","6m","7m","2p","3p","4p","5p","6p","8s","8s"],"#,
            r#"["1m","2m","2m","2m","3m","3m","3m","4m","4m","4m","5m","5m","5m"],"#,
            r#"["6m","6m","6m","7m","7m","7m","8m","8m","8m","8m","9m","1p","1p"]]}"#
        )
        .to_owned(),
    ];
    // A go-around first, so the declaration is an ordinary riichi rather than
    // a double riichi.
    for actor in 0..4 {
        lines.push(format!(r#"{{"type":"tsumo","actor":{actor},"pai":"5z"}}"#));
        lines.push(format!(
            r#"{{"type":"dahai","actor":{actor},"pai":"5z","tsumogiri":true}}"#
        ));
    }
    lines.extend([
        r#"{"type":"tsumo","actor":0,"pai":"3z"}"#.to_owned(),
        r#"{"type":"reach","actor":0}"#.to_owned(),
        r#"{"type":"dahai","actor":0,"pai":"7p","tsumogiri":false}"#.to_owned(),
        // All Simples + Pinfu, 2 han 30 fu, so 2,000 from the dealer plus the
        // 1,000 deposit that the declaration put on the table.
        concat!(
            r#"{"type":"hora","actor":1,"target":0,"pai":"7p","fu":30,"fan":2,"#,
            r#""hora_tehais":["2m","3m","4m","5m","6m","7m","2p","3p","4p","5p","6p","8s","8s"],"#,
            r#""hora_points":3000,"deltas":[-3000,3000,0,0],"#,
            r#""scores":[22000,28000,25000,25000]}"#
        )
        .to_owned(),
        r#"{"type":"end_kyoku"}"#.to_owned(),
    ]);

    let log = crate::from_json_lines(&lines.join("\n")).expect("the fixture should parse");
    let report = audit_log(&log, Settings::default());
    assert_eq!(report.wins_checked, 1);
    assert!(report.is_clean(), "{:?}", report.findings);
}

/// mjai carries no rule set, so the importer has to be told which table it is
/// reading. The presets have to actually differ where the sites differ.
#[test]
fn the_rule_presets_differ_where_the_sites_differ() {
    let tenhou = LogSource::Tenhou.settings();
    let soul = LogSource::MahjongSoul.settings();

    assert!(!tenhou.double_yakuman);
    assert!(soul.double_yakuman);
    for rules in [&tenhou, &soul] {
        assert!(rules.red_fives);
        assert!(rules.opened_all_inside);
        assert!(
            !rules.kiriage_mangan,
            "neither site rounds up to a limit hand"
        );
        assert!(!rules.three_player);
    }
}

/// An empty log is a no-op, not a panic: a converter can legitimately produce
/// one for a game that was abandoned before the first hand.
#[test]
fn an_empty_log_audits_clean() {
    let report = audit_log(&[], Settings::default());
    assert_eq!(report.hands, 0);
    assert!(report.is_clean());
}
