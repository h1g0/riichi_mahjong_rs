//! Unit tests for `Round`.

use super::*;

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

use crate::protocol::DrawReason;

#[test]
fn test_round_new() {
    let round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    assert_eq!(round.round_wind, Wind::East);
    assert_eq!(round.current_player, 0);
    assert_eq!(round.phase, TurnPhase::Draw);
    assert!(round.result.is_none());

    for i in 0..4 {
        assert_eq!(round.players[i].hand.tiles().len(), 13);
    }

    assert_eq!(round.players[0].seat_wind, Wind::East);
}

#[test]
fn test_round_draw() {
    // Seeded wall guarantees the first draw is not a nine-terminals offer.
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    assert!(round.do_draw());
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.players[0].hand.drawn().is_some());

    // One TileDrawn plus three OtherPlayerDrew = four events.
    let events = round.drain_events();
    assert_eq!(events.len(), 4);
}

fn nagashi_discard(tile: Tile, is_called: bool) -> crate::player::Discard {
    crate::player::Discard {
        tile,
        is_tsumogiri: false,
        is_riichi_declaration: false,
        is_called,
    }
}

#[test]
fn test_nagashi_mangan_single_winner_settlement() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 1, 2, 0, 4, Settings::new());
    round.drain_events();
    round.players[1].discards = vec![
        nagashi_discard(Tile::new(Tile::M1), false),
        nagashi_discard(Tile::new(Tile::Z7), false),
    ];

    round.do_exhaustive_draw();

    assert_eq!(round.get_scores(), [20900, 35300, 22900, 22900]);
    assert_eq!(round.riichi_sticks, 0);
    assert!(matches!(
        round.result,
        Some(RoundResult::NagashiMangan { ref winners }) if winners == &[1]
    ));
    let events = round.drain_events();
    assert_eq!(events.len(), 4);
    assert!(events.iter().all(|(_, event)| matches!(
        event,
        ServerEvent::RoundNagashiMangan {
            winners,
            scores: [20900, 35300, 22900, 22900],
            riichi_sticks: 2,
            ..
        } if winners.len() == 1
            && winners[0].wind == Wind::South
            && winners[0].score_points == 10300
    )));
}

#[test]
fn test_multiple_nagashi_mangan_winners_share_payments_independently() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 1, 1, 0, 4, Settings::new());
    round.drain_events();
    round.players[2].discards = vec![nagashi_discard(Tile::new(Tile::S9), false)];
    round.players[0].discards = vec![nagashi_discard(Tile::new(Tile::Z1), false)];

    round.do_exhaustive_draw();

    // Winners are ordered from the dealer, so East receives the honba and
    // riichi deposit even though West was discovered first in the setup.
    assert_eq!(round.get_scores(), [34300, 18900, 28900, 18900]);
    assert!(matches!(
        round.result,
        Some(RoundResult::NagashiMangan { ref winners }) if winners == &[0, 2]
    ));
    let events = round.drain_events();
    let Some((_, ServerEvent::RoundNagashiMangan { winners, .. })) = events.first() else {
        panic!("expected Nagashi Mangan event");
    };
    assert_eq!(winners.len(), 2);
    assert_eq!(winners[0].wind, Wind::East);
    assert_eq!(winners[0].score_points, 13300);
    assert_eq!(winners[1].wind, Wind::West);
    assert_eq!(winners[1].score_points, 8000);
}

#[test]
fn test_nagashi_mangan_qualification_allows_own_open_call() {
    use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.players[1].discards = vec![nagashi_discard(Tile::new(Tile::M9), false)];
    round.players[1].hand.add_meld(Meld {
        tiles: vec![Tile::new(Tile::P2); 3],
        category: MeldType::Pon,
        from: MeldFrom::Previous,
        called_tile: Some(Tile::new(Tile::P2)),
    });

    assert_eq!(round.nagashi_mangan_players(), vec![1]);

    round.players[1].discards[0].is_called = true;
    assert!(round.nagashi_mangan_players().is_empty());
    round.players[1].discards[0].is_called = false;
    round.players[1]
        .discards
        .push(nagashi_discard(Tile::new(Tile::M2), false));
    assert!(round.nagashi_mangan_players().is_empty());
}

#[test]
fn test_sanma_nagashi_mangan_uses_tsumo_loss() {
    let mut round = sanma_round(42, 0);
    round.drain_events();
    round.players[1].discards = vec![nagashi_discard(Tile::new(Tile::P1), false)];

    round.do_exhaustive_draw();

    assert_eq!(round.get_scores(), [31000, 41000, 33000, 0]);
    assert!(matches!(
        round.result,
        Some(RoundResult::NagashiMangan { ref winners }) if winners == &[1]
    ));
}

#[test]
fn test_round_discard() {
    // Seeded wall guarantees the first draw is not a nine-terminals offer.
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();

    assert!(round.do_discard(None));

    assert!(
        round.phase == TurnPhase::Draw || round.phase == TurnPhase::WaitForCalls,
        "phase should be Draw or WaitForCalls, got: {:?}",
        round.phase
    );

    // Pass everyone through a possible WaitForCalls.
    if round.phase == TurnPhase::WaitForCalls {
        for i in 0..4 {
            if let Some(ref cs) = round.call_state
                && !cs.responded[i]
            {
                round.respond_to_call(i, CallResponse::Pass);
                if round.call_state.is_none() {
                    break;
                }
            }
        }
    }

    assert_eq!(round.phase, TurnPhase::Draw);
    assert_eq!(round.current_player, 1);
}

#[test]
fn test_round_discard_rejects_tile_not_in_hand() {
    // Seeded wall guarantees the first draw is not a nine-terminals offer.
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();

    round.players[0].hand = mahjong_core::hand::Hand::from("123m123p123s1234z 5z");

    assert!(!round.do_discard(Some(Tile::new(Tile::Z7))));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert_eq!(round.players[0].discards.len(), 0);
    assert_eq!(round.players[0].hand.drawn(), Some(Tile::new(Tile::Z5)));
}

#[test]
fn test_round_turn_flow() {
    // Seeded wall for reproducibility.
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    // Run one full go-around.
    for expected_player in 0..4 {
        assert_eq!(round.current_player, expected_player);

        // draw
        round.do_draw();
        if round.phase == TurnPhase::RoundOver {
            break;
        }

        // Decline any nine-terminals offer and continue.
        if round.phase == TurnPhase::WaitForNineTerminals {
            round.do_nine_terminals(expected_player, false);
        }

        // discard
        round.do_discard(None);
        if round.phase == TurnPhase::RoundOver {
            break;
        }

        // Pass everyone through a possible WaitForCalls.
        if round.phase == TurnPhase::WaitForCalls {
            for i in 0..4 {
                if let Some(ref cs) = round.call_state
                    && !cs.responded[i]
                {
                    round.respond_to_call(i, CallResponse::Pass);
                    if round.call_state.is_none() {
                        break;
                    }
                }
            }
            if round.phase == TurnPhase::RoundOver {
                break;
            }
        }
    }

    if round.phase != TurnPhase::RoundOver {
        // Back to the first player after the go-around.
        assert_eq!(round.current_player, 0);
    }
}

#[test]
fn test_round_play_to_end() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.play_to_end();

    assert!(round.is_over());
    assert!(round.result.is_some());
}

#[test]
fn test_round_scores() {
    let round = Round::new(
        Wind::East,
        0,
        [25000, 30000, 20000, 25000],
        0,
        0,
        0,
        4,
        Settings::new(),
    );
    let scores = round.get_scores();
    assert_eq!(scores, [25000, 30000, 20000, 25000]);
}

#[test]
fn test_round_events_on_start() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let events = round.drain_events();

    assert_eq!(events.len(), 4);
    for (i, (player_idx, event)) in events.iter().enumerate() {
        assert_eq!(*player_idx, i);
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                round_wind,
                ..
            } => {
                assert_eq!(hand.len(), 13);
                assert_eq!(*scores, [25000; 4]);
                assert_eq!(*round_wind, Wind::East);
                assert_eq!(*seat_wind, round.players[i].seat_wind);
            }
            _ => panic!("Expected GameStarted event"),
        }
    }
}

#[test]
fn test_wait_for_calls_and_pass() {
    // After a discard, WaitForCalls resolves to Draw once everyone passes.
    // Seeded wall guarantees the first draw is not a nine-terminals offer.
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();
    round.do_discard(None);

    if round.phase == TurnPhase::WaitForCalls {
        for i in 0..4 {
            if let Some(ref cs) = round.call_state
                && !cs.responded[i]
            {
                assert!(round.respond_to_call(i, CallResponse::Pass));
                if round.call_state.is_none() {
                    break;
                }
            }
        }
        assert_eq!(round.phase, TurnPhase::Draw);
    }
}

#[test]
fn test_check_available_calls_offers_pon_but_not_ron_for_5z() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[1].seat_wind;
    let hand = mahjong_core::hand::Hand::from("234678m56p567s55z");
    round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Pon { .. }))
    );
    assert!(
        !call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron))
    );
}

fn player_with_three_open_melds(seat_wind: Wind, tiles: Vec<Tile>) -> Player {
    let mut player = Player::new(seat_wind, tiles, 25000);
    for meld_tiles in [
        [Tile::P1, Tile::P2, Tile::P3],
        [Tile::P4, Tile::P5, Tile::P6],
        [Tile::S7, Tile::S8, Tile::S9],
    ] {
        player.hand.add_meld(Meld {
            tiles: meld_tiles.map(Tile::new).to_vec(),
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(Tile::new(meld_tiles[1])),
        });
    }
    player
}

#[test]
fn test_chi_not_offered_when_swap_calling_leaves_no_legal_discard() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[1].seat_wind;
    round.players[1] = player_with_three_open_melds(
        seat_wind,
        vec![
            Tile::new(Tile::M7),
            Tile::new(Tile::M9),
            Tile::new(Tile::M8),
            Tile::new(Tile::M8),
        ],
    );

    let call_state = round.check_available_calls(Tile::new(Tile::M8), 0);

    assert!(
        !call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Chi { .. }))
    );
}

#[test]
fn test_chi_with_no_post_call_discard_is_allowed_when_swap_calling_is_disabled() {
    let mut settings = Settings::new();
    settings.forbid_swap_calling = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    let seat_wind = round.players[1].seat_wind;
    round.players[1] = player_with_three_open_melds(
        seat_wind,
        vec![
            Tile::new(Tile::M7),
            Tile::new(Tile::M9),
            Tile::new(Tile::M8),
            Tile::new(Tile::M8),
        ],
    );

    let call_state = round.check_available_calls(Tile::new(Tile::M8), 0);

    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Chi { .. }))
    );
}

fn open_tanyao_player(seat_wind: Wind, with_drawn: bool) -> Player {
    use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

    let hand = mahjong_core::hand::Hand::from("56677m66s 5m");
    let mut player = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    player.hand.add_meld(Meld {
        tiles: vec![
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
        ],
        category: MeldType::Chi,
        from: MeldFrom::Previous,
        called_tile: None,
    });
    player.hand.add_meld(Meld {
        tiles: vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
        ],
        category: MeldType::Chi,
        from: MeldFrom::Previous,
        called_tile: None,
    });
    if with_drawn {
        player.draw(hand.drawn().unwrap());
    }
    player
}

#[test]
fn test_open_tanyao_disabled_blocks_tsumo() {
    let mut settings = Settings::new();
    settings.opened_all_inside = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);

    let seat_wind = round.players[0].seat_wind;
    round.players[0] = open_tanyao_player(seat_wind, true);
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;

    assert!(!round.can_tsumo());
    assert!(!round.do_tsumo());
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
}

#[test]
fn test_open_tanyao_disabled_does_not_offer_ron() {
    let mut settings = Settings::new();
    settings.opened_all_inside = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);

    let seat_wind = round.players[1].seat_wind;
    round.players[1] = open_tanyao_player(seat_wind, false);

    let call_state = round.check_available_calls(Tile::new(Tile::M5), 0);
    assert!(
        !call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron)),
        "喰いタンなしではオープン断么九のみのロンを提示しない"
    );
}

#[test]
fn test_do_riichi_requires_tenpai_after_discard() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("123m123p123s45z67m 8m");
    round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    round.players[0].draw(hand.drawn().unwrap());
    round.phase = TurnPhase::WaitForDiscard;
    round.current_player = 0;
    round.drain_events();

    assert!(!round.do_riichi(None));
    assert!(!round.players[0].is_riichi);
    assert_eq!(round.players[0].hand.drawn(), Some(Tile::new(Tile::M8)));

    assert!(round.do_riichi(Some(Tile::new(Tile::Z4))));
    assert!(round.players[0].is_riichi);
}

#[test]
fn test_do_riichi_deducts_score_and_adds_stick() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("123m123p123s45z67m 8m");
    round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    round.players[0].draw(hand.drawn().unwrap());
    round.phase = TurnPhase::WaitForDiscard;
    round.current_player = 0;
    round.drain_events();

    assert!(round.do_riichi(Some(Tile::new(Tile::Z4))));
    assert_eq!(round.players[0].score, 24000);
    assert_eq!(round.riichi_sticks, 1);
}

#[test]
fn test_check_available_calls_offers_daiminkan() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[1].seat_wind;
    let hand = mahjong_core::hand::Hand::from("111m234p567s789m");
    round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::M1), 0);
    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Daiminkan))
    );
}

#[test]
fn test_first_pon_declaration_is_not_overwritten() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    for player_idx in 1..4 {
        let seat_wind = round.players[player_idx].seat_wind;
        let hand = mahjong_core::hand::Hand::from("11m234p567s789m12z");
        round.players[player_idx] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    }

    let call_state = round.check_available_calls(Tile::new(Tile::M1), 0);
    let pon_option = |player_idx: usize| {
        call_state.available_calls[player_idx]
            .iter()
            .find_map(|call| match call {
                AvailableCall::Pon { options } => options.first().copied(),
                _ => None,
            })
            .expect("expected pon option")
    };
    let player_two_option = pon_option(2);
    let player_one_option = pon_option(1);
    round.call_state = Some(call_state);
    round.phase = TurnPhase::WaitForCalls;

    assert!(round.respond_to_call(
        2,
        CallResponse::Pon {
            hand_tile_types: player_two_option,
        },
    ));
    assert!(round.respond_to_call(
        1,
        CallResponse::Pon {
            hand_tile_types: player_one_option,
        },
    ));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    assert_eq!(round.current_player, 2);
    assert_eq!(round.players[2].hand.melds().len(), 1);
    assert!(round.players[1].hand.melds().is_empty());
}

#[test]
fn test_final_discard_offers_only_ron() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[1].seat_wind;
    let hand = mahjong_core::hand::Hand::from("11123m456p789s11z");
    round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    let before_last_tile = round.check_available_calls(Tile::new(Tile::M1), 0);
    assert!(
        before_last_tile.available_calls[1]
            .iter()
            .any(|call| !matches!(call, AvailableCall::Ron))
    );
    while round.wall.draw().is_some() {}

    let call_state = round.check_available_calls(Tile::new(Tile::M1), 0);

    assert!(
        call_state.available_calls[1]
            .iter()
            .all(|call| matches!(call, AvailableCall::Ron))
    );
}

#[test]
fn test_do_ankan_draws_rinshan_and_reveals_dora() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("111m234p567s789m 1m");
    round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    round.players[0].draw(hand.drawn().unwrap());
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.drain_events();
    let remaining_before_kan = round.wall.remaining();

    assert!(round.do_kan(Tile::M1));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.players[0].hand.drawn().is_some());
    assert_eq!(round.players[0].hand.melds().len(), 1);
    assert_eq!(round.wall.dora_indicators().len(), 2);
    assert_eq!(round.wall.remaining(), remaining_before_kan - 1);
}

#[test]
fn test_kan_is_rejected_after_the_final_live_wall_draw() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("111m234p567s789m 1m");
    round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    round.players[0].draw(hand.drawn().unwrap());
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    while round.wall.draw().is_some() {}

    assert!(!round.do_kan(Tile::M1));
    assert!(round.players[0].hand.melds().is_empty());
    assert_eq!(round.players[0].hand.drawn(), Some(Tile::new(Tile::M1)));
}

#[test]
fn test_rinshan_win_does_not_also_award_last_tile_draw() {
    use mahjong_core::scoring::score::ScoreItem;

    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("123m456p789s1112z 2z");
    round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    round.players[0].draw(hand.drawn().unwrap());
    // A real kan ends the uninterrupted first turn. Without this, the
    // synthetic setup scores Tenhou and suppresses ordinary yaku.
    round.players[0].is_first_turn = false;
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.last_draw_was_dead_wall = true;
    while round.wall.draw().is_some() {}
    round.drain_events();

    assert!(round.do_tsumo());

    let events = round.drain_events();
    let Some((_, ServerEvent::RoundWon { yaku_list, .. })) = events.first() else {
        panic!("expected win event");
    };
    assert!(yaku_list.iter().any(|(item, _)| {
        *item == ScoreItem::Yaku(mahjong_core::winning_hand::name::Kind::AfterAQuad)
    }));
    assert!(!yaku_list.iter().any(|(item, _)| {
        *item == ScoreItem::Yaku(mahjong_core::winning_hand::name::Kind::LastTileDraw)
    }));
}

#[test]
fn test_discard_after_rinshan_does_not_award_last_tile_claim() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[1].seat_wind;
    let hand = mahjong_core::hand::Hand::from("123456m789p234s5z");
    round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    while round.wall.draw().is_some() {}

    round.last_draw_was_dead_wall = true;
    let after_rinshan = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(
        !after_rinshan.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron))
    );

    round.last_draw_was_dead_wall = false;
    let after_live_wall_draw = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(
        after_live_wall_draw.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron))
    );
}

#[test]
fn test_out_of_range_call_response_is_rejected() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.call_state = Some(round.check_available_calls(Tile::new(Tile::M1), 0));
    round.phase = TurnPhase::WaitForCalls;

    assert!(!round.respond_to_call(4, CallResponse::Pass));
    assert!(!round.respond_to_call(usize::MAX, CallResponse::Pass));
}

#[test]
fn test_do_kakan_draws_rinshan_and_reveals_dora() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let mut player = Player::new(seat_wind, vec![], 25000);
    player.hand = mahjong_core::hand::Hand::from("234p567s789m1z 111m 1m");
    round.players[0] = player;
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.drain_events();

    assert!(round.do_kan(Tile::M1));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.players[0].hand.drawn().is_some());
    assert_eq!(
        round.players[0].hand.melds()[0].category,
        mahjong_core::hand_info::meld::MeldType::Kakan
    );
    assert_eq!(round.wall.dora_indicators().len(), 2);
}

#[test]
fn test_do_kakan_keeps_unrelated_drawn_tile_in_hand() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let mut player = Player::new(seat_wind, vec![], 25000);
    player.hand = mahjong_core::hand::Hand::from("127m234p567s1z 111m 9s");
    round.players[0] = player;
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.drain_events();

    assert!(round.do_kan(Tile::M1));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.players[0].hand.drawn().is_some());
    assert_eq!(round.players[0].hand.tiles().len(), 10);
    assert!(
        round.players[0]
            .hand
            .tiles()
            .contains(&mahjong_core::tile::Tile::new(Tile::S9))
    );
}

#[test]
fn test_temporary_furiten_set_on_ron_pass() {
    // Passing an available ron sets temporary furiten.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    // Player 1 tenpai: 123m456p789s11z waiting on 1z (East round).
    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);

    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|c| matches!(c, AvailableCall::Ron)),
        "player 1 should be able to ron"
    );

    round.phase = TurnPhase::WaitForCalls;
    round.call_state = Some(call_state);
    for i in 0..4 {
        if let Some(ref cs) = round.call_state
            && !cs.responded[i]
        {
            round.respond_to_call(i, CallResponse::Pass);
            if round.call_state.is_none() {
                break;
            }
        }
    }

    assert!(round.players[1].is_temporary_furiten);
    assert!(!round.players[1].is_riichi_furiten);
}

#[test]
fn test_temporary_furiten_cleared_on_draw() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    round.players[1].is_temporary_furiten = true;

    round.current_player = 1;
    round.phase = TurnPhase::Draw;
    round.do_draw();

    // The player's own draw lifts temporary furiten.
    assert!(!round.players[1].is_temporary_furiten);
}

#[test]
fn test_riichi_furiten_set_on_ron_pass() {
    // A riichi player passing a ron becomes riichi-furiten.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);
    round.players[1].is_riichi = true;

    let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);
    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|c| matches!(c, AvailableCall::Ron)),
        "riichi player should be able to ron"
    );

    round.phase = TurnPhase::WaitForCalls;
    round.call_state = Some(call_state);
    for i in 0..4 {
        if let Some(ref cs) = round.call_state
            && !cs.responded[i]
        {
            round.respond_to_call(i, CallResponse::Pass);
            if round.call_state.is_none() {
                break;
            }
        }
    }

    assert!(round.players[1].is_riichi_furiten);
    assert!(!round.players[1].is_temporary_furiten);
}

#[test]
fn test_riichi_furiten_persists_after_draw() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    round.players[1].is_riichi_furiten = true;
    round.players[1].is_riichi = true;

    round.current_player = 1;
    round.phase = TurnPhase::Draw;
    round.do_draw();

    // Riichi furiten survives the player's own draw.
    assert!(round.players[1].is_riichi_furiten);
}

#[test]
fn test_temporary_furiten_blocks_ron() {
    // A furiten player is never offered ron.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);
    round.players[1].is_temporary_furiten = true;

    let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);

    assert!(
        !call_state.available_calls[1]
            .iter()
            .any(|c| matches!(c, AvailableCall::Ron)),
        "furiten player should not be offered ron"
    );
}

#[test]
fn test_kakan_ron_pass_sets_furiten() {
    // Passing a robbing-a-quad chance also sets furiten.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat0 = round.players[0].seat_wind;
    let mut player0 = Player::new(seat0, vec![], 25000);
    player0.hand = mahjong_core::hand::Hand::from("234p567s789m1z 111m 1m");
    round.players[0] = player0;

    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("11m234p567p789s55z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.drain_events();

    assert!(round.do_kan(Tile::M1));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);
    let call_state = round.call_state.as_ref().unwrap();
    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron))
    );

    assert!(round.respond_to_call(1, CallResponse::Pass));
    assert!(round.players[1].is_temporary_furiten);
}

#[test]
fn test_riichi_with_specific_tenpai_hand() {
    // Reproduction: 6m7m1p2p3p3p4p5p5p6p7s8s9s drawing 8m is tenpai with
    // several riichi discards (3p,3p,5p,5p,6p), so can_riichi must be true.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat0 = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("67m12334556p789s");
    round.players[0] = Player::new(seat0, hand.tiles().to_vec(), 25000);
    round.players[0].hand.set_drawn(Some(Tile::new(Tile::M8)));

    assert!(!round.players[0].is_riichi, "should not be in riichi");
    assert!(round.players[0].is_menzen(), "should be menzen");
    assert!(round.players[0].score >= 1000, "should have >= 1000 score");
    assert!(round.wall.remaining() >= 1, "wall should have tiles");
    assert!(
        round.players[0].hand.drawn().is_some(),
        "should have drawn tile"
    );

    assert!(
        round.can_player_riichi(0),
        "should be able to declare riichi with tenpai hand"
    );
}

#[test]
fn test_kakan_offers_rob_ron() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat0 = round.players[0].seat_wind;
    let mut player0 = Player::new(seat0, vec![], 25000);
    player0.hand = mahjong_core::hand::Hand::from("234p567s789m1z 111m 1m");
    round.players[0] = player0;

    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("11m234p567p789s55z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.drain_events();

    assert!(round.do_kan(Tile::M1));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);
    let call_state = round.call_state.as_ref().unwrap();
    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron))
    );

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert_eq!(round.phase, TurnPhase::RoundOver);
    match round.result {
        Some(RoundResult::Ron {
            ref winners,
            loser,
            winning_tile,
        }) => {
            assert_eq!(winners, &vec![1]);
            assert_eq!(loser, 0);
            assert_eq!(winning_tile, Tile::new(Tile::M1));
        }
        _ => panic!("expected ron result after robbing a quad"),
    }
}

// --- Nine terminals ---

/// Sets up a hand qualifying for nine terminals:
/// 1m9m1p9p1s9s1z2z3z4z5z6z7z (all 13 orphan kinds) plus a drawn tile.
fn setup_nine_terminals_hand(round: &mut Round, player_idx: usize) {
    let seat = round.players[player_idx].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z7z 1m");
    round.players[player_idx] = player;
    round.current_player = player_idx;
    round.phase = TurnPhase::WaitForDiscard;
}

#[test]
fn test_check_nine_terminals_qualifies() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    assert!(round.check_nine_terminals());
}

#[test]
fn test_check_nine_terminals_insufficient_types() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat = round.players[0].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    // Only eight orphan kinds (1m,9m,1p,9p,1s,9s,1z,2z), so no offer.
    player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z5m5p5s5s 1m");
    round.players[0] = player;
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    assert!(!round.check_nine_terminals());
}

#[test]
fn test_check_nine_terminals_after_discard() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    // One discard already made: the offer window has closed.
    round.players[0].discards.push(crate::player::Discard {
        tile: Tile::new(Tile::M5),
        is_tsumogiri: true,
        is_riichi_declaration: false,
        is_called: false,
    });
    assert!(!round.check_nine_terminals());
}

#[test]
fn test_do_nine_terminals_declare() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    round.phase = TurnPhase::WaitForNineTerminals;
    round.drain_events();

    assert!(round.do_nine_terminals(0, true));
    assert_eq!(round.phase, TurnPhase::RoundOver);
    assert!(matches!(round.result, Some(RoundResult::SpecialDraw)));

    let events = round.drain_events();
    let has_round_draw = events.iter().any(|(_idx, e)| {
        matches!(
            e,
            ServerEvent::RoundDraw {
                reason: DrawReason::NineTerminals,
                ..
            }
        )
    });
    assert!(has_round_draw, "九種九牌流局イベントが生成されていない");
}

#[test]
fn test_do_nine_terminals_continue() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    round.phase = TurnPhase::WaitForNineTerminals;
    round.drain_events();

    assert!(round.do_nine_terminals(0, false));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.result.is_none());
}

#[test]
fn test_do_nine_terminals_wrong_player() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    round.phase = TurnPhase::WaitForNineTerminals;

    assert!(!round.do_nine_terminals(1, true));
    assert_eq!(round.phase, TurnPhase::WaitForNineTerminals);
}

#[test]
fn test_do_draw_triggers_nine_terminals_phase() {
    // Put 7z (the 13th orphan kind) on top of the wall;
    // Wall::from_tiles draws from the front.
    let mut wall_tiles: Vec<Tile> = vec![Tile::new(Tile::Z7)];
    // Pad the rest (at least the 14 dead-wall tiles are needed).
    for _ in 0..(70 + 14) {
        wall_tiles.push(Tile::new(Tile::M5));
    }
    let wall = Wall::from_tiles(wall_tiles);

    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.wall = wall;

    // Twelve orphan kinds in hand; the drawn 7z makes thirteen.
    let seat = round.players[0].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z5m");
    round.players[0] = player;
    round.current_player = 0;
    round.phase = TurnPhase::Draw;
    round.drain_events();

    round.do_draw();

    assert_eq!(
        round.phase,
        TurnPhase::WaitForNineTerminals,
        "九種九牌条件達成時にWaitForNineTerminalsになるべき"
    );

    let events = round.drain_events();
    let has_available = events
        .iter()
        .any(|(_idx, e)| matches!(e, ServerEvent::NineTerminalsAvailable));
    assert!(
        has_available,
        "NineTerminalsAvailableイベントが生成されていない"
    );
}

#[test]
fn test_nine_terminals_continue_resends_tile_drawn() {
    // Declining re-sends TileDrawn to prompt the discard: the response
    // to the first TileDrawn was rejected during WaitForNineTerminals,
    // so without the re-send the hand would stall.
    let mut wall_tiles: Vec<Tile> = vec![Tile::new(Tile::Z7)];
    for _ in 0..(70 + 14) {
        wall_tiles.push(Tile::new(Tile::M5));
    }
    let wall = Wall::from_tiles(wall_tiles);

    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.wall = wall;
    let seat = round.players[0].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z5m");
    round.players[0] = player;
    round.current_player = 0;
    round.phase = TurnPhase::Draw;
    round.drain_events();

    round.do_draw();
    round.drain_events();

    assert!(round.do_nine_terminals(0, false));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);

    let events = round.drain_events();
    let resent = events
        .iter()
        .any(|(idx, e)| *idx == 0 && matches!(e, ServerEvent::TileDrawn { .. }));
    assert!(resent, "続行時に TileDrawn が再送されるべき");

    // The re-sent draw allows the discard; the hand moves on.
    assert!(round.do_discard(None));
}

#[test]
fn test_nine_terminals_disabled_by_setting() {
    let mut wall_tiles: Vec<Tile> = vec![Tile::new(Tile::Z7)];
    for _ in 0..(70 + 14) {
        wall_tiles.push(Tile::new(Tile::M5));
    }
    let wall = Wall::from_tiles(wall_tiles);

    let mut settings = Settings::new();
    settings.nine_terminals_draw = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    round.wall = wall;

    let seat = round.players[0].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z5m");
    round.players[0] = player;
    round.current_player = 0;
    round.phase = TurnPhase::Draw;
    round.drain_events();

    round.do_draw();

    // With the rule off, play proceeds straight to the discard phase.
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
}

// --- Triple-ron abortive draw ---

/// Sets up three players able to ron player 0's 5s discard, each with a
/// tanyao hand. All winners score identically (non-dealer, same yaku and
/// fu), which the scoring tests rely on.
fn setup_triple_ron(round: &mut Round) {
    let seat0 = round.players[0].seat_wind;
    let mut p0 = Player::new(seat0, vec![], 25000);
    p0.hand = mahjong_core::hand::Hand::from("234m456m234p456p 5s");
    round.players[0] = p0;

    // Players 1-3: 234m456m234p456p5s waiting on 5s, all tanyao.
    for i in 1..=3 {
        let seat = round.players[i].seat_wind;
        let mut p = Player::new(seat, vec![], 25000);
        p.hand = mahjong_core::hand::Hand::from("234m456m234p456p5s");
        round.players[i] = p;
    }

    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
}

#[test]
fn test_triple_ron_draw_enabled() {
    let mut settings = Settings::new();
    settings.triple_ron_draw = true;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Ron));

    assert_eq!(round.phase, TurnPhase::RoundOver);
    assert!(matches!(round.result, Some(RoundResult::SpecialDraw)));

    let events = round.drain_events();
    let has_triple_ron = events.iter().any(|(_idx, e)| {
        matches!(
            e,
            ServerEvent::RoundDraw {
                reason: DrawReason::TripleRon,
                ..
            }
        )
    });
    assert!(has_triple_ron, "三家和流局イベントが生成されていない");
}

#[test]
fn test_triple_ron_draw_takes_priority_over_multiple_ron() {
    // With both triple_ron_draw and multiple_ron on, the abortive draw
    // must take precedence over a triple win.
    let mut settings = Settings::new();
    settings.triple_ron_draw = true;
    settings.multiple_ron = true;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Ron));

    assert_eq!(round.phase, TurnPhase::RoundOver);
    assert!(
        matches!(round.result, Some(RoundResult::SpecialDraw)),
        "triple_ron_draw が multiple_ron より優先されること"
    );
    let events = round.drain_events();
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        ServerEvent::RoundDraw {
            reason: DrawReason::TripleRon,
            ..
        }
    )));
}

#[test]
fn test_triple_ron_draw_disabled_multiple_ron_disabled_picks_winner() {
    // Both flags off: head bump, a single winner.
    let mut settings = Settings::new();
    settings.triple_ron_draw = false;
    settings.multiple_ron = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Ron));

    // multiple_ron=false: the closest player (1) wins.
    assert_eq!(round.phase, TurnPhase::RoundOver);
    match &round.result {
        Some(RoundResult::Ron { winners, loser, .. }) => {
            assert_eq!(winners, &vec![1]);
            assert_eq!(*loser, 0);
        }
        _ => panic!("ロン結果が期待されたが別の結果: {:?}", round.result),
    }
}

#[test]
fn test_two_ron_no_draw() {
    // Two rons never trigger the abortive draw, even with the flag on.
    let mut settings = Settings::new();
    settings.triple_ron_draw = true;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    // Two rons resolve as a double ron, not a draw.
    assert_eq!(round.phase, TurnPhase::RoundOver);
    match &round.result {
        Some(RoundResult::Ron { winners, loser, .. }) => {
            assert_eq!(winners, &vec![1, 2]);
            assert_eq!(*loser, 0);
        }
        _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
    }
}

#[test]
fn test_two_ron_disabled_picks_winner() {
    // multiple_ron=false: head bump, a single winner.
    let mut settings = Settings::new();
    settings.multiple_ron = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    // multiple_ron=false: only the closest player (1) wins.
    assert_eq!(round.phase, TurnPhase::RoundOver);
    match &round.result {
        Some(RoundResult::Ron { winners, loser, .. }) => {
            assert_eq!(winners, &vec![1]);
            assert_eq!(*loser, 0);
        }
        _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
    }
}

#[test]
fn test_double_ron_both_win() {
    // Default multiple_ron: both win.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    assert_eq!(round.phase, TurnPhase::RoundOver);
    match &round.result {
        Some(RoundResult::Ron { winners, loser, .. }) => {
            assert_eq!(winners, &vec![1, 2], "打順優先順で並んでいること");
            assert_eq!(*loser, 0);
        }
        _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
    }
}

#[test]
fn test_triple_ron_all_win() {
    // multiple_ron on, triple_ron_draw off: all three win.
    let mut settings = Settings::new();
    settings.multiple_ron = true;
    settings.triple_ron_draw = false;
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Ron));

    assert_eq!(round.phase, TurnPhase::RoundOver);
    match &round.result {
        Some(RoundResult::Ron { winners, loser, .. }) => {
            assert_eq!(winners, &vec![1, 2, 3]);
            assert_eq!(*loser, 0);
        }
        _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
    }
}

#[test]
fn test_double_ron_scores() {
    // Double-ron scoring: each winner is paid independently; the honba
    // bonus goes only to the first winner in turn order.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 1, 0, 0, 4, Settings::new()); // honba=1
    setup_triple_ron(&mut round);
    round.drain_events();

    let initial_score_loser = round.players[0].score;
    let initial_score_p1 = round.players[1].score;
    let initial_score_p2 = round.players[2].score;

    assert!(round.do_discard(None));
    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    // Player 1 gets the honba bonus (300); player 2 does not.
    let p1_gain = round.players[1].score - initial_score_p1;
    let p2_gain = round.players[2].score - initial_score_p2;
    assert!(
        p1_gain > p2_gain,
        "最初の和了者が本場ボーナスを得ること: p1={}, p2={}",
        p1_gain,
        p2_gain
    );
    assert_eq!(
        p1_gain - p2_gain,
        300,
        "本場ボーナスの差は1本場=300点であること"
    );

    // The deal-in player pays both.
    let loser_loss = initial_score_loser - round.players[0].score;
    let total_gain = p1_gain + p2_gain;
    assert_eq!(
        loser_loss, total_gain,
        "放銃者の支払いが全和了者の取得合計と一致すること"
    );
}

#[test]
fn test_double_ron_events_generated() {
    // A double ron emits one RoundWon per winner.
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    let events = round.drain_events();
    let won_events: Vec<_> = events
        .iter()
        .filter(|(idx, e)| *idx == 0 && matches!(e, ServerEvent::RoundWon { .. }))
        .collect();
    assert_eq!(
        won_events.len(),
        2,
        "ダブロンで2件のRoundWonイベントが生成されること"
    );
}

#[test]
fn test_multi_ron_riichi_sticks_first_winner_only() {
    // The riichi deposits go only to the first winner.
    let settings = Settings::new();
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 2, 0, 4, settings); // riichi_sticks=2
    setup_triple_ron(&mut round);
    round.drain_events();

    let initial_p1 = round.players[1].score;
    let initial_p2 = round.players[2].score;

    assert!(round.do_discard(None));
    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    let p1_gain = round.players[1].score - initial_p1;
    let p2_gain = round.players[2].score - initial_p2;
    // Player 1 nets 2000 more from the two deposits.
    assert_eq!(
        p1_gain - p2_gain,
        2000,
        "供託2本はプレイヤー1のみ取得: 差は2000点"
    );
    assert_eq!(round.riichi_sticks, 0, "供託棒はすべて消費されること");
}

// --- auto_pass_cpu ---

#[test]
fn test_auto_pass_cpu_no_op_when_wrong_phase() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    assert_eq!(round.phase, TurnPhase::Draw);
    round.auto_pass_cpu(0);
    assert_eq!(round.phase, TurnPhase::Draw);
}

#[test]
fn test_auto_pass_cpu_passes_cpu_players_and_resolves() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    // Give player 1 a hand that can pon 5z.
    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("234678m56p567s55z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(!call_state.responded[1], "player 1 should have pending pon");

    round.phase = TurnPhase::WaitForCalls;
    round.call_state = Some(call_state);

    // human = 0 (the discarder); CPU player 1 auto-passes and the calls resolve.
    round.auto_pass_cpu(0);

    assert!(
        round.call_state.is_none(),
        "all CPUs passed → call should resolve"
    );
    assert_eq!(round.phase, TurnPhase::Draw);
}

#[test]
fn test_auto_pass_cpu_skips_human_player() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    // Give player 1 a hand that can pon 5z.
    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("234678m56p567s55z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(!call_state.responded[1], "player 1 should have pending pon");

    round.phase = TurnPhase::WaitForCalls;
    round.call_state = Some(call_state);

    // human = 1 is skipped, so their response stays pending.
    round.auto_pass_cpu(1);

    assert!(
        round.call_state.is_some(),
        "call should still be pending for human player"
    );
    assert!(
        !round.call_state.as_ref().unwrap().responded[1],
        "human player should not have been auto-passed"
    );
    assert_eq!(round.phase, TurnPhase::WaitForCalls);
}

// ===== Swap-calling restriction (#247) =====

use mahjong_core::hand::Hand;

#[test]
fn test_swap_calling_forbids_genbutsu_after_chi() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Holds 3m4m5m; calling the discarded 3m with [4m,5m] keeps a 3m in hand.
    round.players[1].hand = Hand::from("345m234567p678s1z");

    round.execute_chi(
        1,
        0,
        Tile::new(Tile::M3),
        [Tile::new(Tile::M4), Tile::new(Tile::M5)],
    );

    assert_eq!(round.current_player, 1);
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    // Discarding the same kind as the called tile is rejected.
    assert!(!round.do_discard(Some(Tile::new(Tile::M3))));
    // A non-swap discard is fine.
    assert!(round.do_discard(Some(Tile::new(Tile::P2))));
}

#[test]
fn test_swap_calling_forbids_suji_after_chi() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Holds 4m5m6m; calling the discarded 3m with [4m,5m] keeps a 6m in hand.
    round.players[1].hand = Hand::from("456m234567p678s1z");

    round.execute_chi(
        1,
        0,
        Tile::new(Tile::M3),
        [Tile::new(Tile::M4), Tile::new(Tile::M5)],
    );

    // Discarding the suji swap tile (6m, opposite end of 3-4-5) is rejected.
    assert!(!round.do_discard(Some(Tile::new(Tile::M6))));
    // A non-swap discard is fine.
    assert!(round.do_discard(Some(Tile::new(Tile::P2))));
}

#[test]
fn test_swap_calling_forbids_genbutsu_after_pon() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Holds three 1s; the pon leaves one 1s in hand.
    round.players[1].hand = Hand::from("111s234567p678m1z");

    round.execute_pon(
        1,
        0,
        Tile::new(Tile::S1),
        [Tile::new(Tile::S1), Tile::new(Tile::S1)],
    );

    // Discarding the same kind as the called tile is rejected.
    assert!(!round.do_discard(Some(Tile::new(Tile::S1))));
    // A non-swap discard is fine.
    assert!(round.do_discard(Some(Tile::new(Tile::P2))));
}

#[test]
fn test_swap_calling_disabled_allows_genbutsu_discard() {
    let settings = Settings {
        forbid_swap_calling: false,
        ..Settings::new()
    };
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    round.players[1].hand = Hand::from("456m234567p678s1z");

    round.execute_chi(
        1,
        0,
        Tile::new(Tile::M3),
        [Tile::new(Tile::M4), Tile::new(Tile::M5)],
    );

    // With the rule disabled even the suji swap tile may be discarded.
    assert!(round.do_discard(Some(Tile::new(Tile::M6))));
}

// ===== Three-player mahjong (#257) =====

/// Three-player test settings.
fn sanma_settings() -> Settings {
    Settings {
        three_player: true,
        ..Settings::new()
    }
}

/// Builds a three-player test round.
fn sanma_round(seed: u64, dealer: usize) -> Round {
    Round::new_with_seed(
        seed,
        Wind::East,
        dealer,
        [35000, 35000, 35000, 0],
        0,
        0,
        0,
        3,
        sanma_settings(),
    )
}

#[test]
fn test_sanma_round_setup() {
    let round = sanma_round(42, 0);
    assert_eq!(round.player_count, 3);

    // Seat winds are East/South/West only; no North seat.
    assert_eq!(round.players[0].seat_wind, Wind::East);
    assert_eq!(round.players[1].seat_wind, Wind::South);
    assert_eq!(round.players[2].seat_wind, Wind::West);

    // Seats 0-2 get 13 tiles; the dummy seat has none and zero points.
    for i in 0..3 {
        assert_eq!(round.players[i].hand.tiles().len(), 13);
    }
    assert!(round.players[3].hand.tiles().is_empty());
    assert_eq!(round.players[3].score, 0);

    // 108 - 14 dead wall - 3 x 13 = 55.
    assert_eq!(round.wall.remaining(), 55);

    // No 2m-8m may appear.
    for i in 0..3 {
        for tile in round.players[i].hand.tiles() {
            assert!(
                !(Tile::M2..=Tile::M8).contains(&tile.get()),
                "三麻の手牌に萬子2〜8が含まれている: {:?}",
                tile
            );
        }
    }
}

#[test]
fn test_sanma_wind_assignment_with_dealer_1() {
    let round = sanma_round(42, 1);
    assert_eq!(round.players[1].seat_wind, Wind::East);
    assert_eq!(round.players[2].seat_wind, Wind::South);
    assert_eq!(round.players[0].seat_wind, Wind::West);
}

#[test]
fn test_sanma_game_started_only_for_three_seats() {
    let mut round = sanma_round(42, 0);
    let events = round.drain_events();
    // GameStarted goes to seats 0-2 only.
    let seats: Vec<usize> = events
        .iter()
        .filter(|(_, e)| matches!(e, ServerEvent::GameStarted { .. }))
        .map(|(i, _)| *i)
        .collect();
    assert_eq!(seats, vec![0, 1, 2]);
    for (_, e) in &events {
        if let ServerEvent::GameStarted { three_player, .. } = e {
            assert!(three_player);
        }
    }
}

#[test]
fn test_sanma_turn_rotation_wraps_at_three() {
    let mut round = sanma_round(42, 0);
    round.drain_events();

    // After seat 2's discard the turn wraps to seat 0, skipping the dummy.
    for expected_player in [0usize, 1, 2, 0] {
        assert_eq!(round.current_player, expected_player);
        round.do_draw();
        if round.phase == TurnPhase::WaitForNineTerminals {
            round.do_nine_terminals(expected_player, false);
        }
        round.do_discard(None);
        if round.phase == TurnPhase::WaitForCalls {
            for i in 0..3 {
                if let Some(ref cs) = round.call_state
                    && !cs.responded[i]
                {
                    round.respond_to_call(i, CallResponse::Pass);
                    if round.call_state.is_none() {
                        break;
                    }
                }
            }
        }
        if round.phase == TurnPhase::RoundOver {
            return; // A rare win/draw is fine.
        }
    }
}

#[test]
fn test_sanma_no_chi_offered() {
    let mut round = sanma_round(42, 0);
    // Seat 1 (to seat 0's right) could chii.
    round.players[1].hand = Hand::from("199m45p11223399s1z");

    let call_state = round.check_available_calls(Tile::new(Tile::P3), 0);

    // Chii is never offered in three-player games.
    assert!(
        call_state.available_calls[1]
            .iter()
            .all(|c| !matches!(c, AvailableCall::Chi { .. })),
        "三麻でチーが提供された"
    );
}

#[test]
fn test_sanma_pon_still_offered() {
    let mut round = sanma_round(42, 0);
    round.players[2].hand = Hand::from("199m455p1122339s1z");

    let call_state = round.check_available_calls(Tile::new(Tile::P5), 0);

    // Pon is still offered.
    assert!(
        call_state.available_calls[2]
            .iter()
            .any(|c| matches!(c, AvailableCall::Pon { .. })),
        "三麻でポンが提供されない"
    );
    // The dummy seat always counts as responded.
    assert!(call_state.responded[3]);
}

// ===== North extraction (#257 Phase 3) =====

/// Gives the current player a North so pei is possible.
fn setup_pei_round() -> Round {
    let mut round = sanma_round(42, 0);
    round.drain_events();
    round.do_draw();
    round.drain_events();
    // Swap in a hand holding a North; keep the drawn tile from do_draw.
    round.players[0].hand = Hand::from("19m199p1199s1234z 5z");
    round
}

#[test]
fn test_pei_basic_flow() {
    let mut round = setup_pei_round();
    let remaining_before = round.wall.remaining();

    assert!(round.do_pei());

    // The North is set aside and the replacement draw restores 13 + 1.
    assert_eq!(round.players[0].pei_tiles.len(), 1);
    assert_eq!(round.players[0].hand.tiles().len(), 13);
    assert!(round.players[0].hand.drawn().is_some());
    // The replacement shrinks the wall by one.
    assert_eq!(round.wall.remaining(), remaining_before - 1);
    // Still the same player's discard.
    assert_eq!(round.current_player, 0);
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);

    // PeiDeclared reaches all three players with the right counts.
    let events = round.drain_events();
    let pei_events: Vec<_> = events
        .iter()
        .filter(|(_, e)| matches!(e, ServerEvent::PeiDeclared { .. }))
        .collect();
    assert_eq!(pei_events.len(), 3);
    for (_, e) in &pei_events {
        if let ServerEvent::PeiDeclared { player, pei_counts } = e {
            assert_eq!(*player, Wind::East);
            assert_eq!(*pei_counts, [1, 0, 0, 0]);
        }
    }
    // The declarer receives the replacement TileDrawn.
    assert!(
        events
            .iter()
            .any(|(i, e)| *i == 0 && matches!(e, ServerEvent::TileDrawn { .. }))
    );
}

#[test]
fn test_pei_rejected_when_nuki_dora_disabled() {
    let settings = Settings {
        three_player: true,
        nuki_dora: false,
        ..Settings::new()
    };
    let mut round = Round::new_with_seed(
        42,
        Wind::East,
        0,
        [35000, 35000, 35000, 0],
        0,
        0,
        0,
        3,
        settings,
    );
    round.drain_events();
    round.do_draw();
    round.players[0].hand = Hand::from("19m199p1199s1234z 5z");

    assert!(!round.do_pei());
    assert!(round.players[0].pei_tiles.is_empty());
}

#[test]
fn test_pei_rejected_in_four_player_game() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.players[0].hand = Hand::from("19m199p1199s1234z 5z");

    assert!(!round.do_pei());
}

#[test]
fn test_pei_rejected_when_wall_empty() {
    let mut round = setup_pei_round();
    while round.wall.draw().is_some() {}

    assert!(!round.do_pei());
    assert!(round.players[0].pei_tiles.is_empty());
}

#[test]
fn test_pei_win_adds_pei_dora() {
    let mut round = setup_pei_round();
    assert!(round.do_pei());
    round.drain_events();

    // A tanyao tsumo hand (circles/bamboos only: no 2m-8m in sanma).
    // One North already extracted, so the win gains one pei-dora han.
    round.players[0].hand = Hand::from("234567p234567s8s 8s");
    // Avoid Blessing of Heaven: yakuman hands ignore dora.
    round.players[0].is_first_turn = false;

    assert!(round.do_tsumo());

    let events = round.drain_events();
    let won = events
        .iter()
        .find(|(_, e)| matches!(e, ServerEvent::RoundWon { .. }));
    let Some((_, ServerEvent::RoundWon { yaku_list, .. })) = won else {
        panic!("RoundWon イベントがない");
    };
    assert!(
        yaku_list.iter().any(|(item, han)| *item
            == mahjong_core::scoring::score::ScoreItem::Dora(
                mahjong_core::scoring::score::DoraLabel::PeiDora
            )
            && *han == 1),
        "北ドラが加算されていない: {:?}",
        yaku_list
    );
}

// ===== Yakuman liability payment (pao, #134) =====

/// Feeding the third dragon triplet records the feeder as liable.
#[test]
fn test_pao_recorded_on_third_dragon_pon() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Player 1 has ponned White and Green and holds two Red.
    round.players[1].hand = Hand::from("34m88s77z 555z 666z");

    round.execute_pon(
        1,
        0,
        Tile::new(Tile::Z7),
        [Tile::new(Tile::Z7), Tile::new(Tile::Z7)],
    );

    assert_eq!(round.pao[1], vec![(Kind::BigDragons, 0)]);
}

/// The second dragon pon records no liability.
#[test]
fn test_pao_not_recorded_on_second_dragon_pon() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Player 1 has ponned only White.
    round.players[1].hand = Hand::from("34m88s66z77z 555z");

    round.execute_pon(
        1,
        0,
        Tile::new(Tile::Z6),
        [Tile::new(Tile::Z6), Tile::new(Tile::Z6)],
    );

    assert!(round.pao[1].is_empty());
}

/// Feeding the fourth wind triplet records the feeder as liable.
#[test]
fn test_pao_recorded_on_fourth_wind_pon() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Player 1 has ponned East/South/West and holds two North.
    round.players[1].hand = Hand::from("88s44z 111z 222z 333z");

    round.execute_pon(
        1,
        2,
        Tile::new(Tile::Z4),
        [Tile::new(Tile::Z4), Tile::new(Tile::Z4)],
    );

    assert_eq!(round.pao[1], vec![(Kind::BigWinds, 2)]);
}

/// Feeding the fourth quad via a called quad records the feeder.
#[test]
fn test_pao_recorded_on_fourth_kan_daiminkan() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    // Player 1 has three kans and holds three East.
    round.players[1].hand = Hand::from("44p111z 1111m 2222s 9999p");

    round.execute_daiminkan(1, 0, Tile::new(Tile::Z1));

    assert_eq!(round.pao[1], vec![(Kind::FourQuads, 0)]);
}

/// With yakuman_pao off nothing is recorded.
#[test]
fn test_pao_not_recorded_when_disabled() {
    let settings = Settings {
        yakuman_pao: false,
        ..Settings::new()
    };
    let mut round = Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    round.players[1].hand = Hand::from("34m88s77z 555z 666z");

    round.execute_pon(
        1,
        0,
        Tile::new(Tile::Z7),
        [Tile::new(Tile::Z7), Tile::new(Tile::Z7)],
    );

    assert!(round.pao[1].is_empty());
}

/// Big Dragons tsumo with pao: the liable player pays everything.
#[test]
fn test_pao_tsumo_daisangen_full_payment() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.pao[1] = vec![(Kind::BigDragons, 0)];
    round.players[1].hand = Hand::from("34m88s 555z 666z 777z 2m");
    round.players[1].is_first_turn = false;
    round.current_player = 1;
    round.phase = TurnPhase::WaitForDiscard;

    assert!(round.do_tsumo());

    // The liable player 0 covers the whole 32000.
    assert_eq!(round.players[0].score, 25000 - 32000);
    assert_eq!(round.players[1].score, 25000 + 32000);
    assert_eq!(round.players[2].score, 25000);
    assert_eq!(round.players[3].score, 25000);
}

/// Big Dragons ron off a third player: the deal-in player and the
/// liable player split.
#[test]
fn test_pao_ron_daisangen_split_payment() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.pao[1] = vec![(Kind::BigDragons, 0)];
    round.players[1].hand = Hand::from("34m88s 555z 666z 777z");
    round.players[1].is_first_turn = false;

    round.execute_ron(vec![1], 3, Tile::new(Tile::M2), false);

    // 32000 split between deal-in player 3 and liable player 0.
    assert_eq!(round.players[0].score, 25000 - 16000);
    assert_eq!(round.players[1].score, 25000 + 32000);
    assert_eq!(round.players[2].score, 25000);
    assert_eq!(round.players[3].score, 25000 - 16000);
}

/// Deal-in by the liable player: an ordinary full-payment ron.
#[test]
fn test_pao_ron_from_pao_player_pays_full_amount() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.pao[1] = vec![(Kind::BigDragons, 0)];
    round.players[1].hand = Hand::from("34m88s 555z 666z 777z");
    round.players[1].is_first_turn = false;

    round.execute_ron(vec![1], 0, Tile::new(Tile::M2), false);

    assert_eq!(round.players[0].score, 25000 - 32000);
    assert_eq!(round.players[1].score, 25000 + 32000);
    assert_eq!(round.players[2].score, 25000);
    assert_eq!(round.players[3].score, 25000);
}

/// Winning with a different yakuman than the recorded pao pays normally.
#[test]
fn test_pao_not_applied_for_unrelated_yakuman() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    // A Four Quads pao is recorded, but the win is Big Dragons.
    round.pao[1] = vec![(Kind::FourQuads, 0)];
    round.players[1].hand = Hand::from("34m88s 555z 666z 777z 2m");
    round.players[1].is_first_turn = false;
    round.current_player = 1;
    round.phase = TurnPhase::WaitForDiscard;

    assert!(round.do_tsumo());

    // Normal tsumo payments: dealer 16000, others 8000.
    assert_eq!(round.players[0].score, 25000 - 16000);
    assert_eq!(round.players[1].score, 25000 + 32000);
    assert_eq!(round.players[2].score, 25000 - 8000);
    assert_eq!(round.players[3].score, 25000 - 8000);
}

#[test]
fn test_sanma_three_winds_draw() {
    let mut round = sanma_round(42, 0);
    round.drain_events();

    // All players discard the same wind (East) on their first discard.
    for i in 0..3 {
        round.players[i].discards.push(crate::player::Discard {
            tile: Tile::new(Tile::Z1),
            is_tsumogiri: false,
            is_riichi_declaration: false,
            is_called: false,
        });
    }
    assert!(round.check_four_winds_draw(), "三麻の四風連打が成立しない");
}

/// A hand-discard TileDiscarded carries the pre-discard sorted position.
#[test]
fn test_tedashi_discard_event_includes_hand_index() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();

    // Sorted hand: 1m2m3m 1p2p3p 1s2s3s 1z2z3z4z; 1p sits at index 3.
    round.players[0].hand = Hand::from("123m123p123s1234z 5z");
    assert!(round.do_discard(Some(Tile::new(Tile::P1))));

    let events = round.drain_events();
    let discarded = events
        .iter()
        .find_map(|(_, e)| match e {
            crate::protocol::ServerEvent::TileDiscarded {
                is_tsumogiri,
                hand_index,
                ..
            } => Some((*is_tsumogiri, *hand_index)),
            _ => None,
        })
        .expect("TileDiscarded イベントがない");
    assert_eq!(discarded, (false, Some(3)));
}

/// A tsumogiri TileDiscarded carries no hand position.
#[test]
fn test_tsumogiri_discard_event_has_no_hand_index() {
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();

    assert!(round.do_discard(None));

    let events = round.drain_events();
    let discarded = events
        .iter()
        .find_map(|(_, e)| match e {
            crate::protocol::ServerEvent::TileDiscarded {
                is_tsumogiri,
                hand_index,
                ..
            } => Some((*is_tsumogiri, *hand_index)),
            _ => None,
        })
        .expect("TileDiscarded イベントがない");
    assert_eq!(discarded, (true, None));
}
