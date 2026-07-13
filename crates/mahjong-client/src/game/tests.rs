//! Unit tests for the game state.

use super::*;
use mahjong_server::table::GameLength;

#[test]
fn test_set_local_players_assigns_cpu_labels_to_seats_1_to_3() {
    let mut state = GameState::new();
    let configs = [
        CpuConfig::new(CpuLevel::Weak, CpuPersonality::Defensive),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy),
    ];
    state.set_local_players(&configs);

    assert_eq!(state.my_seat, 0);
    assert!(matches!(state.player_labels[0], PlayerLabel::Me));
    assert_eq!(state.player_labels[0].detail(0, Lang::Ja), None);
    assert_eq!(
        state.player_labels[1].detail(1, Lang::Ja),
        Some("CPU1（弱い・守備的）".to_string())
    );
    assert_eq!(
        state.player_labels[2].name(2, Lang::Ja),
        "CPU2（強い・高得点）".to_string()
    );
    assert_eq!(
        state.player_labels[2].name(2, Lang::En),
        "CPU2 (Strong, High Value)".to_string()
    );
}

#[test]
fn test_set_online_players_keeps_seat_order_and_self() {
    let mut state = GameState::new();
    let labels = [
        PlayerLabel::Human("ホスト".to_string()),
        PlayerLabel::Me,
        PlayerLabel::Cpu {
            level: "Normal".to_string(),
            personality: "Speedy".to_string(),
        },
        PlayerLabel::Cpu {
            level: "Normal".to_string(),
            personality: "HighValue".to_string(),
        },
    ];
    state.set_online_players(&labels, 1);

    assert_eq!(state.my_seat, 1);
    assert!(matches!(state.player_labels[1], PlayerLabel::Me));
    assert_eq!(
        state.player_labels[0].detail(3, Lang::Ja),
        Some("ホスト".to_string())
    );
    assert_eq!(
        state.player_labels[2].detail(1, Lang::Ja),
        Some("CPU1（普通・スピード）".to_string())
    );
}

fn game_started_4p(seat_wind: Wind, round_number: usize) -> ServerEvent {
    ServerEvent::GameStarted {
        seat_wind,
        hand: vec![Tile::new(Tile::P1); 13],
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P5)],
        round_number,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    }
}

#[test]
fn test_initial_dealer_seat_derived_from_game_started() {
    // East 1: if we (seat 0) are South, the starting dealer is seat 3.
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::South, 0));
    assert_eq!(state.initial_dealer_seat, 3);

    // East 3 (round_number 2): we at seat 1 as West puts the current
    // dealer at seat 3; rewinding two hands gives starting dealer seat 1.
    let mut state = GameState::new();
    state.my_seat = 1;
    state.handle_event(game_started_4p(Wind::West, 2));
    assert_eq!(state.initial_dealer_seat, 1);
}

#[test]
fn test_final_rankings_tie_breaks_by_dealer_proximity() {
    // With equal scores the order runs counter-clockwise from the
    // starting dealer.
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::West, 0)); // starting dealer: seat 2
    state.scores = [25000; 4];
    let order: Vec<usize> = state.final_rankings().iter().map(|r| r.0).collect();
    assert_eq!(order, vec![2, 3, 0, 1]);

    // Differing scores outrank the seat order.
    state.scores = [30000, 20000, 25000, 25000];
    let order: Vec<usize> = state.final_rankings().iter().map(|r| r.0).collect();
    assert_eq!(order, vec![0, 2, 3, 1]);
}

#[test]
fn test_final_rankings_sanma_excludes_dummy_seat() {
    // Three-player: starting dealer at seat 1 and equal scores give
    // 1, 2, 0, skipping the dummy.
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::West)); // we as West puts the dealer at seat 1
    state.scores = [35000, 35000, 35000, 0];
    let order: Vec<usize> = state.final_rankings().iter().map(|r| r.0).collect();
    assert_eq!(order, vec![1, 2, 0]);
}

fn sanma_game_started(seat_wind: Wind) -> ServerEvent {
    sanma_game_started_at(seat_wind, 0)
}

fn sanma_game_started_at(seat_wind: Wind, round_number: usize) -> ServerEvent {
    ServerEvent::GameStarted {
        seat_wind,
        hand: vec![Tile::new(Tile::P1); 13],
        scores: [35000, 35000, 35000, 0],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P5)],
        round_number,
        total_rounds: 3,
        honba: 0,
        riichi_sticks: 0,
        three_player: true,
        nuki_dora: true,
    }
}

#[test]
fn test_sanma_initial_wind_index_is_fixed_across_rounds() {
    // 東1局: 自分（座席0）が西家 → 開始時の風インデックスは西（2）
    let mut state = GameState::new();
    state.handle_event(sanma_game_started_at(Wind::West, 0));
    assert_eq!(state.my_initial_wind_index(), 2);

    // 東2局: 自分の風は南へ回るが、開始時の風インデックスは変わらない
    // （描画スロットはこの値で固定され、各家の表示位置が動かない）
    state.handle_event(sanma_game_started_at(Wind::South, 1));
    assert_eq!(state.seat_wind, Some(Wind::South));
    assert_eq!(state.my_initial_wind_index(), 2);

    // 東3局: 自分が親（東家）になっても同様
    state.handle_event(sanma_game_started_at(Wind::East, 2));
    assert_eq!(state.my_initial_wind_index(), 2);
}

/// 三麻の鳴き方向は「東1局開始時の風の差分」で決まること（#311 の回帰テスト）。
///
/// 席は東1局開始時の位置で固定表示されるため（#309）、現在の局の風で
/// mod 4 の差分を取ると、風が回った局で倒す牌の位置が画面上の鳴き元の
/// 席とずれる。
#[test]
fn test_sanma_meld_direction_uses_initial_winds() {
    // 自分（座席0）が東1局で東家: 下家（開始時南家）は画面右、
    // 上家（開始時西家）は画面対面に固定表示される。
    let mut state = GameState::new();
    state.handle_event(sanma_game_started_at(Wind::East, 0));
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::South),
        MeldFrom::Following
    );
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::West),
        MeldFrom::Opposite
    );

    // 東2局: 自分は西家に回り、画面右の相手は東家、画面対面の相手は南家になる。
    // 現在の風で計算すると右の相手が Opposite・対面の相手が Following になり、
    // 倒す位置が席と一致しなくなる。
    state.handle_event(sanma_game_started_at(Wind::West, 1));
    assert_eq!(
        state.compute_meld_direction(Wind::West, Wind::East),
        MeldFrom::Following
    );
    assert_eq!(
        state.compute_meld_direction(Wind::West, Wind::South),
        MeldFrom::Opposite
    );

    // 東3局: 自分は南家、画面右は西家、画面対面は東家。
    state.handle_event(sanma_game_started_at(Wind::South, 2));
    assert_eq!(
        state.compute_meld_direction(Wind::South, Wind::West),
        MeldFrom::Following
    );
    assert_eq!(
        state.compute_meld_direction(Wind::South, Wind::East),
        MeldFrom::Opposite
    );
}

/// 他家同士の鳴きでも、倒す位置が固定表示の席の位置関係と一致すること。
#[test]
fn test_sanma_meld_direction_between_opponents() {
    // 自分が東1局で東家、東2局で西家に回った局面。
    // 画面右の相手（開始時南家）は東家、画面対面の相手（開始時西家）は南家。
    let mut state = GameState::new();
    state.handle_event(sanma_game_started_at(Wind::East, 0));
    state.handle_event(sanma_game_started_at(Wind::West, 1));

    // 画面右の相手が自分から鳴く: 自分（画面下）は右の席から見て上家の位置
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::West),
        MeldFrom::Previous
    );
    // 画面対面の相手が画面右の相手から鳴く: 対面の席から見て右の席は上家の位置
    assert_eq!(
        state.compute_meld_direction(Wind::South, Wind::East),
        MeldFrom::Previous
    );
}

/// 四麻の鳴き方向は従来どおり現在の風の差分で決まること（挙動不変の確認）。
#[test]
fn test_4p_meld_direction_uses_current_winds() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::North),
        MeldFrom::Previous
    );
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::West),
        MeldFrom::Opposite
    );
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::South),
        MeldFrom::Following
    );

    // 四麻は風の差分 mod 4 が局によらず不変なので、局が進んでも結果は同じ
    state.handle_event(game_started_4p(Wind::North, 1));
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::North),
        MeldFrom::Previous
    );
    assert_eq!(
        state.compute_meld_direction(Wind::South, Wind::East),
        MeldFrom::Previous
    );
}

#[test]
fn test_sanma_game_started_sets_player_count() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));

    assert_eq!(state.player_count, 3);
    assert!(state.is_three_player());
    assert!(state.nuki_dora);
    assert_eq!(state.pei_counts, [0; 4]);
}

#[test]
fn test_sanma_relative_player_index_wraps_at_three() {
    let mut state = GameState::new();
    // With us as West, East sits to our right (relative 1).
    state.handle_event(sanma_game_started(Wind::West));

    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::East,
        tile: Tile::new(Tile::P3),
        is_tsumogiri: false,
        hand_index: None,
    });
    assert_eq!(
        state.discards[1].len(),
        1,
        "三麻で西家から見た東家は下家（相対1）のはず"
    );
}

#[test]
fn test_sanma_pei_declared_updates_counts() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.other_players[0].concealed_count = 13;

    state.handle_event(ServerEvent::PeiDeclared {
        player: Wind::South,
        pei_counts: [0, 1, 0, 0],
    });

    assert_eq!(state.pei_counts, [0, 1, 0, 0]);
    // The extracted North briefly leaves South one hidden tile short.
    assert_eq!(state.other_players[0].concealed_count, 12);
}

#[test]
fn test_sanma_can_pei_with_north_in_hand() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.hand = vec![Tile::new(Tile::Z4)];

    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::P2),
        remaining_tiles: 50,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(state.can_pei, "手牌に北があるのに北抜き不可");

    // Under riichi a North in the hand alone is not extractable.
    state.is_riichi = true;
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::P2),
        remaining_tiles: 49,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(!state.can_pei, "リーチ中の手牌北で北抜き可になっている");

    // A drawn North is.
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 48,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(state.can_pei, "リーチ中のツモ北で北抜き不可");
}

/// Regression: a North drawn under riichi must offer pei instead of
/// being auto-discarded. The auto-tsumogiri used to fire without
/// checking pei, leaving the pei button visible but unclickable.
#[test]
fn test_riichi_drawn_north_holds_auto_discard_for_pei() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.is_riichi = true;

    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 48,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(state.can_pei, "リーチ中のツモ北で北抜き不可");

    // While pei is available the auto-discard stays held even past
    // its delay (#291).
    assert!(state.handle_input(None, 100.0).is_none());
    assert!(
        state
            .handle_input(None, 100.0 + RIICHI_AUTO_DISCARD_SECS * 2.0)
            .is_none()
    );
    assert!(state.drawn.is_some(), "自動ツモ切りが発火した");

    let action = state.handle_input(
        Some(crate::renderer::OverlayClick::Action(ClientAction::Pei)),
        100.0 + RIICHI_AUTO_DISCARD_SECS * 2.0,
    );
    assert!(matches!(action, Some(ClientAction::Pei)));
}

/// Passing the pei offer under riichi falls back to the usual
/// automatic tsumogiri.
#[test]
fn test_riichi_pei_pass_discards_drawn_north() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.is_riichi = true;

    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 48,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(state.can_pei);

    // The pass discards immediately, with no delay.
    let action = state.handle_input(Some(crate::renderer::OverlayClick::PassSelfCall), 100.0);
    assert!(matches!(action, Some(ClientAction::Discard { tile: None })));
    assert!(state.drawn.is_none(), "ツモ切り後もツモ牌が残っている");
    assert!(!state.can_pei);
}

/// Regression (#296): no pei button on the last draw (empty wall).
/// The server rejects pei without a replacement draw, so the button
/// would be dead.
#[test]
fn test_sanma_no_pei_on_last_draw() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.hand = vec![Tile::new(Tile::Z4)];

    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 0,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(!state.can_pei, "海底ツモで北抜き可になっている");
}

#[test]
fn test_setup_state_build_game_settings() {
    let mut setup = SetupState::new();
    let settings = setup.build_game_settings();
    assert!(!settings.rules.three_player);
    assert_eq!(settings.length, GameLength::EastOnly, "既定は東風戦");
    assert_eq!(setup.cpu_count(), 3);

    setup.mode = GameMode::ThreeHanchan;
    setup.nuki_dora = false;
    let settings = setup.build_game_settings();
    assert!(settings.rules.three_player);
    assert!(!settings.rules.nuki_dora);
    assert_eq!(
        settings.length,
        GameLength::Hanchan,
        "半荘戦が length に反映されない"
    );
    assert_eq!(settings.initial_score, 35000);
    assert_eq!(setup.cpu_count(), 2);
}

/// Round-trip between game mode and (three-player flag, length).
#[test]
fn test_game_mode_parts_roundtrip() {
    let expected = [
        (GameMode::FourEast, false, GameLength::EastOnly),
        (GameMode::FourHanchan, false, GameLength::Hanchan),
        (GameMode::ThreeEast, true, GameLength::EastOnly),
        (GameMode::ThreeHanchan, true, GameLength::Hanchan),
    ];
    // ALL matches the mode toggle's display order.
    assert_eq!(GameMode::ALL.map(|m| m), expected.map(|(m, _, _)| m));
    for (mode, three_player, length) in expected {
        assert_eq!(mode.three_player(), three_player, "{mode:?}");
        assert_eq!(mode.length(), length, "{mode:?}");
        assert_eq!(GameMode::from_parts(three_player, length), mode);
    }
}

#[test]
fn test_called_tile_is_marked_in_river() {
    let mut state = GameState::new();
    state.seat_wind = Some(Wind::East);
    let tile = Tile::new(Tile::P3);

    // South discards; the tile joins the pool uncalled.
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile,
        is_tsumogiri: false,
        hand_index: None,
    });
    assert_eq!(state.discards[1].len(), 1);
    assert!(!state.discards[1][0].is_called);

    // West's pon marks the tile in South's pool as called.
    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::West,
        call_type: CallType::Pon,
        called_tile: tile,
        tiles: vec![tile, tile],
    });
    assert!(state.discards[1][0].is_called);
}

#[test]
fn test_called_tile_marked_despite_stale_call_offer() {
    let mut state = GameState::new();
    state.seat_wind = Some(Wind::East);
    // A stale call_discarder from an earlier offer (after passing).
    state.call_discarder = Some(Wind::North);

    let tile = Tile::new(Tile::S5);
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile,
        is_tsumogiri: false,
        hand_index: None,
    });
    // West's chii must attribute to the latest discarder,
    // not the stale call_discarder.
    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::West,
        call_type: CallType::Chi,
        called_tile: tile,
        tiles: vec![Tile::new(Tile::S6), Tile::new(Tile::S7)],
    });
    assert!(state.discards[1][0].is_called);
}

#[test]
fn test_self_chi_sets_forbidden_swap_discards_and_clears_on_discard() {
    let mut state = GameState::new();
    state.seat_wind = Some(Wind::East);
    state.last_discarder = Some(Wind::North);

    // Chii the left player's 3m with 4m/5m (sequence 3-4-5).
    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::East,
        call_type: CallType::Chi,
        called_tile: Tile::new(Tile::M3),
        tiles: vec![
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::M5),
        ],
    });

    // Both the called kind (3m) and the suji tile (6m) become forbidden.
    assert!(state.forbidden_discards.contains(&Tile::M3));
    assert!(state.forbidden_discards.contains(&Tile::M6));

    // Completing a discard lifts the restriction.
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::East,
        tile: Tile::new(Tile::P1),
        is_tsumogiri: false,
        hand_index: None,
    });
    assert!(state.forbidden_discards.is_empty());
}

#[test]
fn test_self_pon_forbids_only_called_tile() {
    let mut state = GameState::new();
    state.seat_wind = Some(Wind::East);
    state.last_discarder = Some(Wind::North);

    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::East,
        call_type: CallType::Pon,
        called_tile: Tile::new(Tile::S1),
        tiles: vec![Tile::new(Tile::S1); 3],
    });

    assert_eq!(state.forbidden_discards, vec![Tile::S1]);
}

#[test]
fn test_self_daiminkan_has_no_forbidden_discards() {
    let mut state = GameState::new();
    state.seat_wind = Some(Wind::East);
    state.last_discarder = Some(Wind::North);

    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::East,
        call_type: CallType::Daiminkan,
        called_tile: Tile::new(Tile::S1),
        tiles: vec![Tile::new(Tile::S1); 4],
    });

    assert!(state.forbidden_discards.is_empty());
}

#[test]
fn test_enter_riichi_selection_marks_only_tenpai_discards() {
    let mut state = GameState::new();
    let hand = Hand::from("123m123p123s45z67m 8m");
    state.hand = hand.tiles().to_vec();
    state.hand.sort();
    state.drawn = hand.drawn();
    state.enter_riichi_selection();

    assert_eq!(state.riichi_selectable_tiles.len(), 2);
    assert_eq!(
        state.hand[state.riichi_selectable_tiles[0]],
        Tile::new(Tile::Z4)
    );
    assert_eq!(
        state.hand[state.riichi_selectable_tiles[1]],
        Tile::new(Tile::Z5)
    );
    assert!(!state.riichi_selectable_drawn);
}

#[test]
fn test_can_discard_for_riichi_rejects_non_tenpai_discard() {
    let mut state = GameState::new();
    let hand = Hand::from("123m123p123s45z67m 8m");
    state.hand = hand.tiles().to_vec();
    state.hand.sort();
    state.drawn = hand.drawn();

    assert!(!state.can_discard_for_riichi(None));
    assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::Z4))));
    assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::Z5))));
}

#[test]
fn test_can_discard_for_riichi_after_ankan_uses_opened_melds() {
    let mut state = GameState::new();
    let hand = Hand::from("1m1m5m5m7m7m9m1s2s3s 3m3m3m3m 8m");
    state.hand = hand.tiles().to_vec();
    state.hand.sort();
    state.drawn = hand.drawn();
    state.melds.push(Meld {
        category: MeldType::Kan,
        tiles: vec![
            Tile::new(Tile::M3),
            Tile::new(Tile::M3),
            Tile::new(Tile::M3),
            Tile::new(Tile::M3),
        ],
        from: MeldFrom::Myself,
        called_tile: None,
    });

    assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::M5))));
    assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::M7))));
}

// --- Declaration banners ---

fn queued_pon(player: Wind) -> ServerEvent {
    let tile = Tile::new(Tile::P3);
    ServerEvent::PlayerCalled {
        player,
        call_type: CallType::Pon,
        called_tile: tile,
        tiles: vec![tile, tile, tile],
    }
}

fn round_won_tsumo(winner: Wind) -> ServerEvent {
    ServerEvent::RoundWon {
        winner,
        loser: None,
        winning_tile: Tile::new(Tile::P1),
        scores: [25000; 4],
        yaku_list: vec![],
        han: 1,
        fu: 30,
        score_points: 1000,
        rank: mahjong_core::scoring::score::ScoreRank::Normal,
        has_opened: false,
        uradora_indicators: vec![],
        riichi_sticks: 0,
        player_hands: vec![],
    }
}

fn round_won_ron(winner: Wind, loser: Wind) -> ServerEvent {
    match round_won_tsumo(winner) {
        ServerEvent::RoundWon {
            winner,
            winning_tile,
            scores,
            yaku_list,
            han,
            fu,
            score_points,
            rank,
            has_opened,
            uradora_indicators,
            riichi_sticks,
            player_hands,
            ..
        } => ServerEvent::RoundWon {
            winner,
            loser: Some(loser),
            winning_tile,
            scores,
            yaku_list,
            han,
            fu,
            score_points,
            rank,
            has_opened,
            uradora_indicators,
            riichi_sticks,
            player_hands,
        },
        _ => unreachable!(),
    }
}

#[test]
fn test_call_banner_shown_and_following_events_held() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    // South's pon shows a banner; the pon itself applies at once.
    state.queue_event(queued_pon(Wind::South));
    state.queue_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 60,
    });
    state.process_events(100.0);

    assert!(matches!(
        state.call_banners[1],
        Some(CallBanner {
            label: Key::Pon,
            ..
        })
    ));
    assert_eq!(state.other_players[0].melds.len(), 1);
    // The following OtherPlayerDrew is held; the count is stale.
    assert_eq!(state.remaining_tiles, 70);

    // It applies once the hold ends.
    state.process_events(100.0 + CALL_HOLD_SECS);
    assert_eq!(state.remaining_tiles, 60);
}

#[test]
fn test_riichi_banner_holds_declaration_discard() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    state.queue_event(ServerEvent::PlayerRiichi {
        player: Wind::West,
        scores: [25000, 25000, 24000, 25000],
        riichi_sticks: 1,
    });
    state.queue_event(ServerEvent::TileDiscarded {
        player: Wind::West,
        tile: Tile::new(Tile::M1),
        is_tsumogiri: true,
        hand_index: None,
    });
    state.process_events(100.0);

    // Riichi applies at once (scores, deposits); the declaration
    // discard is held.
    assert!(matches!(
        state.call_banners[2],
        Some(CallBanner {
            label: Key::Riichi,
            ..
        })
    ));
    assert_eq!(state.riichi_sticks, 1);
    assert!(state.discards[2].is_empty());

    state.process_events(100.0 + CALL_HOLD_SECS);
    assert_eq!(state.discards[2].len(), 1);
    assert!(state.discards[2][0].is_riichi);
}

#[test]
fn test_round_won_deferred_until_banner_hold_elapses() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    state.queue_event(round_won_tsumo(Wind::South));
    state.process_events(100.0);

    // Only the tsumo banner shows; the result transition is held.
    assert!(matches!(
        state.call_banners[1],
        Some(CallBanner {
            label: Key::Tsumo,
            ..
        })
    ));
    assert_eq!(state.phase, GamePhase::Playing);

    // Reprocessing during the hold must not transition.
    state.process_events(100.0 + WIN_HOLD_SECS / 2.0);
    assert_eq!(state.phase, GamePhase::Playing);

    // The result screen appears once the hold ends.
    state.process_events(100.0 + WIN_HOLD_SECS);
    assert_eq!(state.phase, GamePhase::RoundResult);
}

#[test]
fn test_double_ron_shows_banners_for_all_winners() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    state.queue_event(round_won_ron(Wind::South, Wind::East));
    state.queue_event(round_won_ron(Wind::West, Wind::East));
    state.process_events(100.0);

    // Both ron banners show together.
    assert!(matches!(
        state.call_banners[1],
        Some(CallBanner {
            label: Key::Ron,
            ..
        })
    ));
    assert!(matches!(
        state.call_banners[2],
        Some(CallBanner {
            label: Key::Ron,
            ..
        })
    ));
    assert_eq!(state.phase, GamePhase::Playing);

    // Both RoundWon events apply after the hold: two result pages.
    state.process_events(100.0 + WIN_HOLD_SECS);
    assert_eq!(state.phase, GamePhase::RoundResult);
    assert_eq!(state.win_results.len(), 2);
}

#[test]
fn test_banner_expires_after_display_duration() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    state.queue_event(queued_pon(Wind::South));
    state.process_events(100.0);
    assert!(state.call_banners[1].is_some());

    state.process_events(100.0 + CALL_BANNER_SECS);
    assert!(state.call_banners[1].is_none());
}

#[test]
fn test_banners_cleared_on_new_round() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    state.queue_event(queued_pon(Wind::South));
    state.process_events(100.0);
    assert!(state.call_banners[1].is_some());

    state.queue_event(game_started_4p(Wind::East, 1));
    state.process_events(100.0 + CALL_HOLD_SECS);
    assert!(state.call_banners[1].is_none());
}

/// Regression: the HandUpdated right after our own PlayerCalled must
/// apply in the same frame, not wait out the banner hold. Holding it
/// let a discard made meanwhile be rolled back by the late HandUpdated,
/// permanently desyncing the hand (the discarded tile resurrected).
#[test]
fn test_own_call_applies_hand_update_in_same_frame() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    // Our own pon: the server sends PlayerCalled then HandUpdated.
    state.queue_event(queued_pon(Wind::East));
    let new_hand = vec![Tile::new(Tile::M1); 11];
    state.queue_event(ServerEvent::HandUpdated {
        hand: new_hand.clone(),
    });
    state.queue_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 60,
    });
    state.process_events(100.0);

    // Both apply in one frame; the hand is current.
    assert_eq!(state.melds.len(), 1);
    assert_eq!(state.hand, new_hand);
    assert!(state.is_my_turn);

    // The banner hold itself remains; further events stay held.
    assert_eq!(state.remaining_tiles, 70);
    state.process_events(100.0 + CALL_HOLD_SECS);
    assert_eq!(state.remaining_tiles, 60);
}

/// Regression: input must be refused while unapplied events remain.
/// A discard made against the stale screen would be rolled back by the
/// pending events and desync from the server.
#[test]
fn test_input_blocked_while_events_pending() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    // The riichi auto-discard fires without mouse input.
    state.is_my_turn = true;
    state.is_riichi = true;
    state.drawn = Some(Tile::new(Tile::M1));

    // No input (including the auto-discard) while events are queued.
    state.queue_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 60,
    });
    assert!(state.handle_input(None, 100.0).is_none());
    assert!(state.drawn.is_some());

    // Draining the queue re-enables the discard (after its delay).
    state.process_events(100.0);
    assert!(state.handle_input(None, 100.0).is_none());
    assert!(matches!(
        state.handle_input(None, 100.0 + RIICHI_AUTO_DISCARD_SECS),
        Some(ClientAction::Discard { tile: None })
    ));
}

/// Regression (#291): the riichi auto-discard waits with the drawn tile
/// visible before firing; it used to discard instantly, hiding the tile.
#[test]
fn test_riichi_auto_discard_waits_before_firing() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.is_riichi = true;
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::M1),
        remaining_tiles: 60,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });

    // During the delay no discard happens and the tile stays visible.
    assert!(state.handle_input(None, 100.0).is_none());
    assert!(state.drawn.is_some());
    assert!(
        state
            .handle_input(None, 100.0 + RIICHI_AUTO_DISCARD_SECS / 2.0)
            .is_none()
    );
    assert!(state.drawn.is_some());

    // Past the delay the tsumogiri fires.
    assert!(matches!(
        state.handle_input(None, 100.0 + RIICHI_AUTO_DISCARD_SECS),
        Some(ClientAction::Discard { tile: None })
    ));
    assert!(state.drawn.is_none());
}

/// The auto-discard delay must restart on every draw; a leftover
/// deadline from the previous turn would discard the next draw
/// instantly.
#[test]
fn test_riichi_auto_discard_timer_resets_on_new_draw() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.is_riichi = true;
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::M1),
        remaining_tiles: 60,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });

    // First turn: wait, then fire.
    assert!(state.handle_input(None, 100.0).is_none());
    assert!(
        state
            .handle_input(None, 100.0 + RIICHI_AUTO_DISCARD_SECS)
            .is_some()
    );

    // Second draw: the delay restarts; no instant discard.
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::M2),
        remaining_tiles: 56,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    let now = 110.0;
    assert!(state.handle_input(None, now).is_none());
    assert!(state.drawn.is_some());
    assert!(
        state
            .handle_input(None, now + RIICHI_AUTO_DISCARD_SECS)
            .is_some()
    );
}

#[test]
fn test_win_announcement_collapses_stale_action_ui() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.available_calls = vec![AvailableCall::Ron];
    state.can_tsumo = true;

    // Stale ron/tsumo buttons collapse during the ron hold.
    state.queue_event(round_won_ron(Wind::East, Wind::South));
    state.process_events(100.0);
    assert!(state.available_calls.is_empty());
    assert!(!state.can_tsumo);
    assert_eq!(state.phase, GamePhase::Playing);
}

// --- Opponent hand display (drawn-tile overhang, gap animation) ---

/// An opponent's draw must only set the overhang flag, not change the
/// tile count - keeping the centered hand from shifting (regression).
#[test]
fn test_other_player_draw_marks_drawn_without_moving_hand() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    state.handle_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 69,
    });

    let other = &state.other_players[0];
    assert!(other.has_drawn);
    assert_eq!(other.concealed_count, 13, "ツモ牌は手牌の枚数に含めない");
}

/// A tsumogiri leaves the hand untouched, with no gap animation.
#[test]
fn test_other_player_tsumogiri_keeps_hand_untouched() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.handle_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 69,
    });

    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::M1),
        is_tsumogiri: true,
        hand_index: None,
    });

    let other = &state.other_players[0];
    assert!(!other.has_drawn);
    assert_eq!(other.concealed_count, 13);
    assert!(other.tedashi_anim.is_none(), "ツモ切りでは詰め演出をしない");
}

/// A hand discard merges the drawn tile in (count unchanged) and starts
/// the gap animation at the vacated slot.
#[test]
fn test_other_player_tedashi_starts_gap_animation() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.queue_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 69,
    });
    state.queue_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::M1),
        is_tsumogiri: false,
        hand_index: Some(4),
    });
    state.process_events(100.0);

    let other = &state.other_players[0];
    assert!(!other.has_drawn);
    assert_eq!(other.concealed_count, 13, "手出しではツモ牌が手牌へ入る");
    let anim = other.tedashi_anim.expect("詰め演出が開始されていない");
    assert_eq!(anim.gap_index, 4);
    assert!(anim.had_drawn);
    assert_eq!(anim.started_at, 100.0);
}

/// A post-call discard (no drawn tile) shrinks the hand by one.
#[test]
fn test_other_player_tedashi_after_call_decrements_count() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.last_discarder = Some(Wind::East);

    // South pons (13 -> 11 tiles), then discards (-> 10).
    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::South,
        call_type: CallType::Pon,
        called_tile: Tile::new(Tile::S1),
        tiles: vec![Tile::new(Tile::S1); 3],
    });
    assert_eq!(state.other_players[0].concealed_count, 11);

    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::M1),
        is_tsumogiri: false,
        hand_index: Some(0),
    });

    let other = &state.other_players[0];
    assert_eq!(other.concealed_count, 10);
    let anim = other.tedashi_anim.expect("詰め演出が開始されていない");
    assert!(!anim.had_drawn);
}

/// An opponent's concealed kan moves four tiles including the drawn one
/// into the meld (regression: the drawn tile used not to be deducted,
/// showing one tile too many).
#[test]
fn test_other_player_ankan_consumes_four_tiles() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.handle_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 69,
    });

    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::South,
        call_type: CallType::Ankan,
        called_tile: Tile::new(Tile::Z5),
        tiles: vec![Tile::new(Tile::Z5); 4],
    });

    let other = &state.other_players[0];
    assert_eq!(other.concealed_count, 10, "13枚＋ツモ1枚から4枚が副露へ");
    assert!(!other.has_drawn);

    // The replacement draw restores the overhang.
    state.handle_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 68,
    });
    assert!(state.other_players[0].has_drawn);
    assert_eq!(state.other_players[0].concealed_count, 10);
}

/// A pei with a drawn tile present keeps the hand count: the North
/// leaves the drawn tile or the hand, and the drawn tile merges in.
#[test]
fn test_sanma_pei_with_drawn_keeps_hand_count() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.handle_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 54,
    });

    state.handle_event(ServerEvent::PeiDeclared {
        player: Wind::South,
        pei_counts: [0, 1, 0, 0],
    });

    let other = &state.other_players[0];
    assert_eq!(other.concealed_count, 13);
    assert!(!other.has_drawn);
}

/// turn_player must track the events; it drives the center panel's
/// turn indicator (#307).
#[test]
fn test_turn_player_tracks_events() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    assert_eq!(state.turn_player, None, "局開始直後は手番未確定");

    // Our own draw makes it our turn.
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::P2),
        remaining_tiles: 69,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert_eq!(state.turn_player, Some(Wind::East));

    // After our discard, South's draw moves the turn.
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::East,
        tile: Tile::new(Tile::P2),
        is_tsumogiri: true,
        hand_index: None,
    });
    state.handle_event(ServerEvent::OtherPlayerDrew {
        player: Wind::South,
        remaining_tiles: 68,
    });
    assert_eq!(state.turn_player, Some(Wind::South));

    // West pons South's discard, taking the turn.
    let tile = Tile::new(Tile::S5);
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile,
        is_tsumogiri: false,
        hand_index: None,
    });
    state.handle_event(ServerEvent::PlayerCalled {
        player: Wind::West,
        call_type: CallType::Pon,
        called_tile: tile,
        tiles: vec![tile, tile],
    });
    assert_eq!(state.turn_player, Some(Wind::West));

    // The draw clears the indicator.
    state.handle_event(ServerEvent::RoundDraw {
        scores: [25000; 4],
        reason: mahjong_server::protocol::DrawReason::Exhaustive,
        tenpai: vec![],
        riichi_sticks: 0,
        player_hands: vec![],
        declarer: None,
    });
    assert_eq!(state.turn_player, None, "流局後も手番表示が残っている");
}
