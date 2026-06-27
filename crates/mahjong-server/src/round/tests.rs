//! `Round` のユニットテスト

use super::*;

#[test]
fn test_round_new() {
    let round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    assert_eq!(round.round_wind, Wind::East);
    assert_eq!(round.current_player, 0);
    assert_eq!(round.phase, TurnPhase::Draw);
    assert!(round.result.is_none());

    // 各プレイヤーに13枚配られている
    for i in 0..4 {
        assert_eq!(round.players[i].hand.tiles().len(), 13);
    }

    // 親（プレイヤー0）が東家
    assert_eq!(round.players[0].seat_wind, Wind::East);
}

#[test]
fn test_round_draw() {
    // 固定シードで牌山を生成し、初回ツモが九種九牌にならないことを保証する
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events(); // 初期イベントをクリア

    assert!(round.do_draw());
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.players[0].hand.drawn().is_some());

    // イベントを確認: 1つのTileDrawn + 3つのOtherPlayerDrew = 4イベント
    let events = round.drain_events();
    assert_eq!(events.len(), 4);
}

#[test]
fn test_round_discard() {
    // 固定シードで牌山を生成し、初回ツモが九種九牌にならないことを保証する
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();

    // ツモ切り
    assert!(round.do_discard(None));

    // 打牌後のフェーズは Draw か WaitForCalls
    assert!(
        round.phase == TurnPhase::Draw || round.phase == TurnPhase::WaitForCalls,
        "phase should be Draw or WaitForCalls, got: {:?}",
        round.phase
    );

    // 鳴き待ちなら全員パスして進める
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
    assert_eq!(round.current_player, 1); // 次のプレイヤーへ
}

#[test]
fn test_round_discard_rejects_tile_not_in_hand() {
    // 固定シードで牌山を生成し、初回ツモが九種九牌にならないことを保証する
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
    // 固定シードで牌山を生成し、テストの再現性を確保する
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    // 4人分のターンを回す
    for expected_player in 0..4 {
        assert_eq!(round.current_player, expected_player);

        // draw
        round.do_draw();
        if round.phase == TurnPhase::RoundOver {
            break;
        }

        // 九種九牌が成立した場合は宣言せず続行する
        if round.phase == TurnPhase::WaitForNineTerminals {
            round.do_nine_terminals(expected_player, false);
        }

        // discard
        round.do_discard(None);
        if round.phase == TurnPhase::RoundOver {
            break;
        }

        // WaitForCalls なら全員パス
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
        // 一巡して最初のプレイヤーに戻る
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

    // 4人分のGameStartedイベント
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
    // 打牌後に WaitForCalls になった場合、全員パスで Draw に進む
    // 固定シードで牌山を生成し、初回ツモが九種九牌にならないことを保証する
    let mut round =
        Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();
    round.do_draw();
    round.drain_events();
    round.do_discard(None);

    if round.phase == TurnPhase::WaitForCalls {
        // 全員パス
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
fn test_do_ankan_draws_rinshan_and_reveals_dora() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat_wind = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("111m234p567s789m 1m");
    round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
    round.players[0].draw(hand.drawn().unwrap());
    round.current_player = 0;
    round.phase = TurnPhase::WaitForDiscard;
    round.drain_events();

    assert!(round.do_kan(Tile::M1));
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.players[0].hand.drawn().is_some());
    assert_eq!(round.players[0].hand.melds().len(), 1);
    assert_eq!(round.wall.dora_indicators().len(), 2);
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
    // プレイヤー1がロン可能な状態で、パスすると同巡フリテンが設定される
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    // プレイヤー1にテンパイ手を設定: 123m456p789s11z 待ち1z（場風東）
    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    // プレイヤー0が1z（東）を捨てた場合をチェック
    let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);

    // ロンが可能であること
    assert!(
        call_state.available_calls[1]
            .iter()
            .any(|c| matches!(c, AvailableCall::Ron)),
        "player 1 should be able to ron"
    );

    // CallStateをセットしてパスで応答
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

    // 同巡フリテンが設定されていること
    assert!(round.players[1].is_temporary_furiten);
    assert!(!round.players[1].is_riichi_furiten);
}

#[test]
fn test_temporary_furiten_cleared_on_draw() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    // プレイヤー1に同巡フリテンを設定
    round.players[1].is_temporary_furiten = true;

    // プレイヤー1のツモ番にする
    round.current_player = 1;
    round.phase = TurnPhase::Draw;
    round.do_draw();

    // 同巡フリテンが解除されていること
    assert!(!round.players[1].is_temporary_furiten);
}

#[test]
fn test_riichi_furiten_set_on_ron_pass() {
    // リーチ中のプレイヤーがロンを見逃すとリーチ後フリテンが設定される
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

    // リーチ後フリテンが設定されていること
    assert!(round.players[1].is_riichi_furiten);
    assert!(!round.players[1].is_temporary_furiten);
}

#[test]
fn test_riichi_furiten_persists_after_draw() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.drain_events();

    // リーチ後フリテンを設定
    round.players[1].is_riichi_furiten = true;
    round.players[1].is_riichi = true;

    // プレイヤー1がツモ
    round.current_player = 1;
    round.phase = TurnPhase::Draw;
    round.do_draw();

    // リーチ後フリテンは解除されないこと
    assert!(round.players[1].is_riichi_furiten);
}

#[test]
fn test_temporary_furiten_blocks_ron() {
    // 同巡フリテンのプレイヤーにはロンが提供されない
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);
    round.players[1].is_temporary_furiten = true;

    let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);

    // フリテンなのでロンが提供されないこと
    assert!(
        !call_state.available_calls[1]
            .iter()
            .any(|c| matches!(c, AvailableCall::Ron)),
        "furiten player should not be offered ron"
    );
}

#[test]
fn test_kakan_ron_pass_sets_furiten() {
    // 加カンで搶槓可能だがパスした場合、フリテンが設定される
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

    // ロンせずパス → フリテンが設定されること
    assert!(round.respond_to_call(1, CallResponse::Pass));
    assert!(round.players[1].is_temporary_furiten);
}

#[test]
fn test_riichi_with_specific_tenpai_hand() {
    // 再現テスト: 6m7m1p2p3p3p4p5p5p6p7s8s9s ツモ8m
    // shanten=0 で riichi_discards がある（3p,3p,5p,5p,6p）
    // → can_riichi = true であるべき
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());

    let seat0 = round.players[0].seat_wind;
    let hand = mahjong_core::hand::Hand::from("67m12334556p789s");
    round.players[0] = Player::new(seat0, hand.tiles().to_vec(), 25000);
    round.players[0].hand.set_drawn(Some(Tile::new(Tile::M8)));

    // 前提条件チェック
    assert!(!round.players[0].is_riichi, "should not be in riichi");
    assert!(round.players[0].is_menzen(), "should be menzen");
    assert!(round.players[0].score >= 1000, "should have >= 1000 score");
    assert!(round.wall.remaining() >= 1, "wall should have tiles");
    assert!(
        round.players[0].hand.drawn().is_some(),
        "should have drawn tile"
    );

    // リーチ可能であるべき
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

// ─── 九種九牌テスト ───────────────────────────────────────────────────────────

/// 九種九牌の条件を満たす手牌をセットアップするヘルパー
///
/// 1m9m1p9p1s9s1z2z3z4z5z6z7z (13種全ヤオ九牌) + ツモ牌1枚
fn setup_nine_terminals_hand(round: &mut Round, player_idx: usize) {
    let seat = round.players[player_idx].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    // 14枚: 1m9m1p9p1s9s1z2z3z4z5z6z7z + ツモ1m（重複は問題なし）
    player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z7z 1m");
    round.players[player_idx] = player;
    round.current_player = player_idx;
    round.phase = TurnPhase::WaitForDiscard;
}

#[test]
fn test_check_nine_terminals_qualifies() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    // 初回ツモ（捨て牌0枚）かつヤオ九牌9種以上
    assert!(round.check_nine_terminals());
}

#[test]
fn test_check_nine_terminals_insufficient_types() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    let seat = round.players[0].seat_wind;
    let mut player = Player::new(seat, vec![], 25000);
    // ヤオ九牌が8種類のみ（6z・7zがなく中張牌が多い）
    // 1m,9m,1p,9p,1s,9s,1z,2z = 8種
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
    // 捨て牌を1枚追加（既に1巡した状態を再現）
    round.players[0].discards.push(crate::player::Discard {
        tile: Tile::new(Tile::M5),
        is_tsumogiri: true,
        is_riichi_declaration: false,
        is_called: false,
    });
    // 捨て牌済みなので宣言不可
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
    // 続行 → 打牌フェーズへ
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    assert!(round.result.is_none());
}

#[test]
fn test_do_nine_terminals_wrong_player() {
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    setup_nine_terminals_hand(&mut round, 0);
    round.phase = TurnPhase::WaitForNineTerminals;

    // 別プレイヤーからのアクションは無効
    assert!(!round.do_nine_terminals(1, true));
    assert_eq!(round.phase, TurnPhase::WaitForNineTerminals);
}

#[test]
fn test_do_draw_triggers_nine_terminals_phase() {
    // 牌山の先頭を7z（13種目のヤオ九牌）に設定する
    // Wall::from_tiles は先頭から draw() するため、先頭に7zを置く
    let mut wall_tiles: Vec<Tile> = vec![Tile::new(Tile::Z7)];
    // 残りは適当な牌で埋める（最低 14 枚の王牌分が必要）
    for _ in 0..(70 + 14) {
        wall_tiles.push(Tile::new(Tile::M5));
    }
    let wall = Wall::from_tiles(wall_tiles);

    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
    round.wall = wall;

    // 手牌をヤオ九牌12種に設定（ツモで7zが来て13種になる）
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
    // 続行を選んだプレイヤーには TileDrawn を再送して打牌を促す。
    // （最初の TileDrawn への打牌は WaitForNineTerminals フェーズで
    // 拒否されているため、再送がないと局が進行不能になる）
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

    // 再送されたツモ牌で打牌できる（局が進行する）
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

    // 設定オフなら通常の打牌フェーズになる
    assert_eq!(round.phase, TurnPhase::WaitForDiscard);
}

// ─── 三家和流局テスト ─────────────────────────────────────────────────────────

/// 3人がロン可能な状態を作るヘルパー
///
/// - プレイヤー0: 打牌側（5sを捨てる）
/// - プレイヤー1,2,3: 5sでロン可能な手牌（タンヤオ形）
///
/// 全員の手牌は同じ点数になる（非親・同一役・同一符）ため点数テストに使える。
fn setup_triple_ron(round: &mut Round) {
    // プレイヤー0: 5sをツモ切り
    let seat0 = round.players[0].seat_wind;
    let mut p0 = Player::new(seat0, vec![], 25000);
    // 12枚クローズ + ツモ牌5s
    p0.hand = mahjong_core::hand::Hand::from("234m456m234p456p 5s");
    round.players[0] = p0;

    // プレイヤー1,2,3: 5sでロン可能な手牌（234m456m234p456p5s で 55s 待ち）
    // 全員タンヤオ（2〜8のみ）で同一役・同一符
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

    // プレイヤー0が1mを捨てる
    assert!(round.do_discard(None));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);

    // 3人全員がロン宣言
    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Ron));

    // 三家和流局になること
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
    // triple_ron_draw=true かつ multiple_ron=true の両方が有効な場合、
    // 三家和流局が優先されてトリロン（全員和了）にはならないことを明示的に確認する
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
    // triple_ron_draw=false, multiple_ron=false の場合は上家取り（頭ハネ）の1人ロン
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

    // multiple_ron=false → 上家（プレイヤー1）が優先してロン
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
    // 2人ロンは三家和流局にならない（triple_ron_draw=true でも2人なら流局しない）
    let mut settings = Settings::new();
    settings.triple_ron_draw = true;
    // multiple_ron=true（デフォルト）なので両方和了
    let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, 4, settings);
    setup_triple_ron(&mut round);
    round.drain_events();

    assert!(round.do_discard(None));
    assert_eq!(round.phase, TurnPhase::WaitForCalls);

    assert!(round.respond_to_call(1, CallResponse::Ron));
    assert!(round.respond_to_call(2, CallResponse::Ron));
    assert!(round.respond_to_call(3, CallResponse::Pass));

    // 2人ロンは流局でなくダブロン
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
    // multiple_ron=false の場合は上家取り（頭ハネ）の1人ロン
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

    // multiple_ron=false → 上家（プレイヤー1）のみロン
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
    // multiple_ron=true（デフォルト）: 2人ロンで両方和了
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
    // multiple_ron=true かつ triple_ron_draw=false: 3人ロンで全員和了
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
    // ダブロン時のスコア: 各和了者が放銃者から独立して点数を受け取る
    // 本場ボーナスは上家取りで最初の和了者（プレイヤー1）のみ
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

    // プレイヤー1: 本場ボーナスあり (honba=1 → 300点加算)
    // プレイヤー2: 本場ボーナスなし
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

    // 放銃者は両方の点数を払う
    let loser_loss = initial_score_loser - round.players[0].score;
    let total_gain = p1_gain + p2_gain;
    assert_eq!(
        loser_loss, total_gain,
        "放銃者の支払いが全和了者の取得合計と一致すること"
    );
}

#[test]
fn test_double_ron_events_generated() {
    // ダブロン時に各和了者分のRoundWonイベントが生成されること
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
    // 供託棒は最初の和了者（プレイヤー1）のみ取得
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
    // プレイヤー1は供託2本（2000点）分多く得点しているはず
    assert_eq!(
        p1_gain - p2_gain,
        2000,
        "供託2本はプレイヤー1のみ取得: 差は2000点"
    );
    assert_eq!(round.riichi_sticks, 0, "供託棒はすべて消費されること");
}

// ─── auto_pass_cpu テスト ────────────────────────────────────────────────────

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

    // プレイヤー1に5zポン可能な手牌をセット
    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("234678m56p567s55z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(!call_state.responded[1], "player 1 should have pending pon");

    round.phase = TurnPhase::WaitForCalls;
    round.call_state = Some(call_state);

    // human = 0 (捨て牌側), CPU のプレイヤー1 が自動パスされて鳴き解決される
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

    // プレイヤー1に5zポン可能な手牌をセット
    let seat1 = round.players[1].seat_wind;
    let hand1 = mahjong_core::hand::Hand::from("234678m56p567s55z");
    round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

    let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
    assert!(!call_state.responded[1], "player 1 should have pending pon");

    round.phase = TurnPhase::WaitForCalls;
    round.call_state = Some(call_state);

    // human = 1 → プレイヤー1はスキップされるので応答が残る
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
