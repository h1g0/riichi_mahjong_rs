//! ゲーム状態のユニットテスト

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
    // 東1局: 自分（座席0）が南家なら起家は座席3
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::South, 0));
    assert_eq!(state.initial_dealer_seat, 3);

    // 東3局（round_number=2）: 座席1の自分が西家なら現在の親は座席3、
    // 2局巻き戻して起家は座席1
    let mut state = GameState::new();
    state.my_seat = 1;
    state.handle_event(game_started_4p(Wind::West, 2));
    assert_eq!(state.initial_dealer_seat, 1);
}

#[test]
fn test_final_rankings_tie_breaks_by_dealer_proximity() {
    // 全員同点なら起家から反時計回りの順になる
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::West, 0)); // 起家は座席2
    state.scores = [25000; 4];
    let order: Vec<usize> = state.final_rankings().iter().map(|r| r.0).collect();
    assert_eq!(order, vec![2, 3, 0, 1]);

    // 点数が異なる場合は点数順が優先される
    state.scores = [30000, 20000, 25000, 25000];
    let order: Vec<usize> = state.final_rankings().iter().map(|r| r.0).collect();
    assert_eq!(order, vec![0, 2, 3, 1]);
}

#[test]
fn test_final_rankings_sanma_excludes_dummy_seat() {
    // 三麻: 起家が座席1のとき、同点ならダミー席を除いて 1, 2, 0 の順
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::West)); // 西家=自分(座席0)なら起家は座席1
    state.scores = [35000, 35000, 35000, 0];
    let order: Vec<usize> = state.final_rankings().iter().map(|r| r.0).collect();
    assert_eq!(order, vec![1, 2, 0]);
}

fn sanma_game_started(seat_wind: Wind) -> ServerEvent {
    ServerEvent::GameStarted {
        seat_wind,
        hand: vec![Tile::new(Tile::P1); 13],
        scores: [35000, 35000, 35000, 0],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P5)],
        round_number: 0,
        total_rounds: 3,
        honba: 0,
        riichi_sticks: 0,
        three_player: true,
        nuki_dora: true,
    }
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
    // 自分が西家（インデックス2）の場合、東家は下家（相対1）になる
    state.handle_event(sanma_game_started(Wind::West));

    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::East,
        tile: Tile::new(Tile::P3),
        is_tsumogiri: false,
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
    // 他家（南家）の手牌枚数を通常の13枚にしておく
    state.other_players[0].concealed_count = 13;

    state.handle_event(ServerEvent::PeiDeclared {
        player: Wind::South,
        pei_counts: [0, 1, 0, 0],
    });

    assert_eq!(state.pei_counts, [0, 1, 0, 0]);
    // 北を抜いた分、南家の伏せ牌は一時的に1枚減って見える
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

    // リーチ中は手牌の北だけでは抜けない
    state.is_riichi = true;
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::P2),
        remaining_tiles: 49,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(!state.can_pei, "リーチ中の手牌北で北抜き可になっている");

    // リーチ中でもツモった牌が北なら抜ける
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 48,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(state.can_pei, "リーチ中のツモ北で北抜き不可");
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

/// 対局モードと（三麻フラグ・対局の長さ）の相互変換
#[test]
fn test_game_mode_parts_roundtrip() {
    let expected = [
        (GameMode::FourEast, false, GameLength::EastOnly),
        (GameMode::FourHanchan, false, GameLength::Hanchan),
        (GameMode::ThreeEast, true, GameLength::EastOnly),
        (GameMode::ThreeHanchan, true, GameLength::Hanchan),
    ];
    // ALL はモードトグルの表示順と一致する
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

    // 南家が捨てる（河に積まれ、まだ鳴かれていない）
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile,
        is_tsumogiri: false,
    });
    assert_eq!(state.discards[1].len(), 1);
    assert!(!state.discards[1][0].is_called);

    // 西家がポン → 南家の河の該当牌が鳴かれた扱いになる
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
    // 過去の鳴き打診で call_discarder が残っている状況（パスした後など）
    state.call_discarder = Some(Wind::North);

    let tile = Tile::new(Tile::S5);
    // 下家（南）が捨てる
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::South,
        tile,
        is_tsumogiri: false,
    });
    // 対面（西）がチー → 古い call_discarder ではなく直前の打牌者を鳴き元とする
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

    // 上家が捨てた 3m を 4m,5m でチー（順子 3-4-5）
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

    // 現物(3m)とスジ(6m)が打牌禁止になる
    assert!(state.forbidden_discards.contains(&Tile::M3));
    assert!(state.forbidden_discards.contains(&Tile::M6));

    // 打牌が完了すると制限が解除される
    state.handle_event(ServerEvent::TileDiscarded {
        player: Wind::East,
        tile: Tile::new(Tile::P1),
        is_tsumogiri: false,
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

// ─── 宣言バナー（鳴き・リーチ・和了の発声表示）のテスト ──────────────────────

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

    // 下家（南）のポン → バナー表示・ポン自体は即適用
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
    // 後続の OtherPlayerDrew は保留され、残り枚数は未更新
    assert_eq!(state.remaining_tiles, 70);

    // 保留明けに適用される
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
    });
    state.process_events(100.0);

    // リーチは即適用（点数・供託が更新）、宣言牌の打牌は保留
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

    // ツモ宣言バナーだけ表示され、結果画面への遷移は保留される
    assert!(matches!(
        state.call_banners[1],
        Some(CallBanner {
            label: Key::Tsumo,
            ..
        })
    ));
    assert_eq!(state.phase, GamePhase::Playing);

    // 保留中は再処理しても遷移しない
    state.process_events(100.0 + WIN_HOLD_SECS / 2.0);
    assert_eq!(state.phase, GamePhase::Playing);

    // 保留明けに結果画面へ
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

    // 2人分のロンバナーが同時に表示される
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

    // 保留明けに両方の RoundWon が適用される（2ページ分の結果）
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

#[test]
fn test_win_announcement_collapses_stale_action_ui() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.available_calls = vec![AvailableCall::Ron];
    state.can_tsumo = true;

    // ロン宣言の保留中は古いロン・ツモボタンを畳む
    state.queue_event(round_won_ron(Wind::East, Wind::South));
    state.process_events(100.0);
    assert!(state.available_calls.is_empty());
    assert!(!state.can_tsumo);
    assert_eq!(state.phase, GamePhase::Playing);
}
