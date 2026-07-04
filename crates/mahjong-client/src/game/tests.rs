//! ゲーム状態のユニットテスト

use super::*;

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
    assert_eq!(settings.round_count, 1, "既定は東風戦");
    assert_eq!(setup.cpu_count(), 3);

    setup.three_player = true;
    setup.hanchan = true;
    setup.nuki_dora = false;
    let settings = setup.build_game_settings();
    assert!(settings.rules.three_player);
    assert!(!settings.rules.nuki_dora);
    assert_eq!(
        settings.round_count, 2,
        "半荘戦が round_count に反映されない"
    );
    assert_eq!(settings.initial_score, 35000);
    assert_eq!(setup.cpu_count(), 2);
}

/// 対局モードのインデックスと三麻・半荘フラグの相互変換
#[test]
fn test_setup_state_mode_index_roundtrip() {
    let mut setup = SetupState::new();
    let expected = [
        (false, false), // 四人東風
        (false, true),  // 四人半荘
        (true, false),  // 三人東風
        (true, true),   // 三人半荘
    ];
    for (idx, &(three_player, hanchan)) in expected.iter().enumerate() {
        setup.set_mode_index(idx);
        assert_eq!(setup.three_player, three_player, "mode {idx}");
        assert_eq!(setup.hanchan, hanchan, "mode {idx}");
        assert_eq!(setup.mode_index(), idx, "mode {idx}");
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
