//! Unit tests for the game state.

use super::*;
use mahjong_core::scoring::score::{DoraLabel, ScoreItem, ScoreRank};
use mahjong_core::winning_hand::name::Kind;
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
    // East 1: we (seat 0) are West, so our initial wind index is 2.
    let mut state = GameState::new();
    state.handle_event(sanma_game_started_at(Wind::West, 0));
    assert_eq!(state.my_initial_wind_index(), 2);

    // East 2: our current wind rotates to South, but the initial wind
    // index remains fixed so each seat stays in the same screen slot.
    state.handle_event(sanma_game_started_at(Wind::South, 1));
    assert_eq!(state.seat_wind, Some(Wind::South));
    assert_eq!(state.my_initial_wind_index(), 2);

    // East 3: the index remains fixed even when we become East.
    state.handle_event(sanma_game_started_at(Wind::East, 2));
    assert_eq!(state.my_initial_wind_index(), 2);
}

/// Sanma meld direction is based on the wind difference at the start
/// of East 1 (regression test for #311).
///
/// Seats remain in their East 1 screen positions (#309). Using the
/// current winds modulo four after winds rotate would place the sideways
/// tile away from the caller's displayed source seat.
#[test]
fn test_sanma_meld_direction_uses_initial_winds() {
    // We (seat 0) start East 1 as East. The initial South is fixed on
    // the right, and the initial West is fixed across the screen.
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

    // In East 2 we rotate to West; the right opponent is now East and
    // the opponent across is South. Current winds would incorrectly map
    // them to Opposite and Following respectively.
    state.handle_event(sanma_game_started_at(Wind::West, 1));
    assert_eq!(
        state.compute_meld_direction(Wind::West, Wind::East),
        MeldFrom::Following
    );
    assert_eq!(
        state.compute_meld_direction(Wind::West, Wind::South),
        MeldFrom::Opposite
    );

    // In East 3 we are South, the right seat is West, and across is East.
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

/// Calls between opponents also orient the sideways tile according to
/// their fixed screen positions.
#[test]
fn test_sanma_meld_direction_between_opponents() {
    // We start East 1 as East and rotate to West in East 2. The right
    // opponent (initial South) is now East; the opponent across is South.
    let mut state = GameState::new();
    state.handle_event(sanma_game_started_at(Wind::East, 0));
    state.handle_event(sanma_game_started_at(Wind::West, 1));

    // The right opponent calls from us at the bottom: we are their
    // previous player on screen.
    assert_eq!(
        state.compute_meld_direction(Wind::East, Wind::West),
        MeldFrom::Previous
    );
    // The opponent across calls from the right opponent, who is their
    // previous player on screen.
    assert_eq!(
        state.compute_meld_direction(Wind::South, Wind::East),
        MeldFrom::Previous
    );
}

/// Four-player meld direction continues to use current-wind differences.
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

    // In four-player games wind differences modulo four remain stable
    // as hands advance.
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
fn test_game_started_uses_authoritative_mode_and_clears_turn_state() {
    let mut state = GameState::new();
    state.is_my_turn = true;
    state.forbidden_discards.push(Tile::M1);
    state.selected_forbidden_swap = true;

    let mut event = sanma_game_started(Wind::East);
    let ServerEvent::GameStarted { total_rounds, .. } = &mut event else {
        unreachable!("helper always returns GameStarted")
    };
    *total_rounds = 6;
    state.handle_event(event);

    assert_eq!(state.setup_state.mode, GameMode::ThreeHanchan);
    assert!(state.setup_state.rules.nuki_dora);
    assert!(!state.is_my_turn);
    assert!(state.forbidden_discards.is_empty());
    assert!(!state.selected_forbidden_swap);

    let mut event = game_started_4p(Wind::East, 0);
    let ServerEvent::GameStarted { total_rounds, .. } = &mut event else {
        unreachable!("helper always returns GameStarted")
    };
    *total_rounds = 8;
    state.handle_event(event);
    assert_eq!(state.setup_state.mode, GameMode::FourHanchan);
}

#[test]
fn test_self_turn_action_hides_stale_controls_until_server_event() {
    let mut state = GameState::new();
    state.handle_event(sanma_game_started(Wind::East));
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 48,
        can_tsumo: true,
        can_riichi: true,
        is_furiten: false,
    });
    state.self_kan_options.push(Tile::new(Tile::P1));
    assert!(state.is_my_turn);
    assert!(state.can_pei);

    let action = state.handle_input(
        Some(crate::renderer::OverlayClick::Action(ClientAction::Tsumo)),
        100.0,
    );
    assert!(matches!(action, Some(ClientAction::Tsumo)));
    assert!(!state.is_my_turn);
    assert!(!state.can_tsumo);
    assert!(!state.can_riichi);
    assert!(!state.can_pei);
    assert!(state.self_kan_options.is_empty());
    assert!(
        state
            .handle_input(
                Some(crate::renderer::OverlayClick::Action(ClientAction::Tsumo)),
                100.1,
            )
            .is_none(),
        "a stale overlay click resubmitted the action"
    );

    // A server event authoritatively opens the next available action.
    state.handle_event(ServerEvent::TileDrawn {
        tile: Tile::new(Tile::P2),
        remaining_tiles: 47,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    assert!(state.is_my_turn);
}

#[test]
fn test_hand_selection_is_informational_outside_our_turn() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.hand = vec![Tile::new(Tile::M1), Tile::new_red(Tile::M5)];
    state.is_my_turn = false;

    assert!(state.handle_hand_tile_click(1).is_none());
    assert_eq!(state.selected_tile, Some(1));
    assert_eq!(state.selected_tile_type(), Some(Tile::M5));
    assert_eq!(state.hand.len(), 2);

    assert!(state.handle_hand_tile_click(1).is_none());
    assert_eq!(state.selected_tile, Some(1));
    assert_eq!(state.hand.len(), 2);
}

#[test]
fn test_hand_selection_still_discards_on_second_click_during_our_turn() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.hand = vec![Tile::new(Tile::M1), Tile::new(Tile::M2)];
    state.drawn = Some(Tile::new(Tile::P9));
    state.is_my_turn = true;

    assert!(state.handle_hand_tile_click(1).is_none());
    let action = state.handle_hand_tile_click(1);

    assert!(matches!(
        action,
        Some(ClientAction::Discard {
            tile: Some(tile)
        }) if tile.get() == Tile::M2
    ));
    assert!(!state.is_my_turn);
    assert_eq!(state.hand, vec![Tile::new(Tile::M1), Tile::new(Tile::P9)]);
    assert_eq!(
        state
            .self_tedashi_anim
            .as_ref()
            .expect("手出しアニメーションが開始されていない")
            .origins,
        vec![SelfTileOrigin::Hand(0), SelfTileOrigin::Drawn]
    );
}

#[test]
fn test_local_hand_discard_tracks_origins_through_sorting() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
    ];
    state.drawn = Some(Tile::new(Tile::M1));
    state.process_events(100.0);

    let discarded = state.apply_local_discard_from_hand(1);

    assert_eq!(discarded, Tile::new(Tile::M3));
    assert_eq!(
        state.hand,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M4),
        ]
    );
    let anim = state
        .self_tedashi_anim
        .as_ref()
        .expect("手出しアニメーションが開始されていない");
    assert_eq!(anim.pre_hand_len, 3);
    assert_eq!(anim.started_at, 100.0);
    assert_eq!(
        anim.origins,
        vec![
            SelfTileOrigin::Drawn,
            SelfTileOrigin::Hand(0),
            SelfTileOrigin::Hand(2),
        ]
    );
}

#[test]
fn test_local_tsumogiri_does_not_start_hand_animation() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.drawn = Some(Tile::new(Tile::P9));
    state.is_my_turn = true;
    state.selected_drawn = true;

    let action = state.handle_drawn_tile_click();

    assert!(matches!(action, Some(ClientAction::Discard { tile: None })));
    assert!(state.drawn.is_none());
    assert!(state.self_tedashi_anim.is_none());
}

#[test]
fn test_authoritative_hand_update_clears_local_hand_animation() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.hand = vec![Tile::new(Tile::M1), Tile::new(Tile::M2)];
    state.drawn = Some(Tile::new(Tile::P9));
    state.apply_local_discard_from_hand(0);
    assert!(state.self_tedashi_anim.is_some());

    state.handle_event(ServerEvent::HandUpdated {
        hand: vec![Tile::new(Tile::S1), Tile::new(Tile::S2)],
    });

    assert!(state.self_tedashi_anim.is_none());
}

#[test]
fn test_selected_drawn_tile_type_is_available_outside_our_turn() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.drawn = Some(Tile::new_red(Tile::P5));
    state.is_my_turn = false;

    assert!(state.handle_drawn_tile_click().is_none());
    assert!(state.selected_drawn);
    assert_eq!(state.selected_tile_type(), Some(Tile::P5));
    assert!(state.drawn.is_some());
}

#[test]
fn test_clearing_tile_selection_also_clears_related_warnings() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.selected_tile = Some(0);
    state.selected_drawn = true;
    state.selected_forbidden_swap = true;
    state.selected_would_cause_furiten = true;

    state.clear_tile_selection();

    assert_eq!(state.selected_tile, None);
    assert!(!state.selected_drawn);
    assert!(!state.selected_forbidden_swap);
    assert!(!state.selected_would_cause_furiten);
}

#[test]
fn test_overlay_click_clears_selected_tile() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));
    state.selected_tile = Some(0);
    state.available_calls = vec![AvailableCall::Ron];

    let action = state.handle_input(
        Some(crate::renderer::OverlayClick::Action(ClientAction::Pass)),
        100.0,
    );

    assert!(matches!(action, Some(ClientAction::Pass)));
    assert_eq!(state.selected_tile, None);
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
    assert!(settings.rules.double_yakuman);
    assert_eq!(settings.length, GameLength::EastOnly, "既定は東風戦");
    assert_eq!(setup.cpu_count(), 3);

    setup.mode = GameMode::ThreeHanchan;
    setup.rules.nuki_dora = false;
    setup.rules.tsumo_loss = false;
    setup.rules.double_yakuman = false;
    setup.rules.opened_all_inside = false;
    let settings = setup.build_game_settings();
    assert!(settings.rules.three_player);
    assert!(!settings.rules.nuki_dora);
    assert!(!settings.rules.tsumo_loss);
    assert!(!settings.rules.double_yakuman);
    assert!(!settings.rules.opened_all_inside);
    assert_eq!(
        settings.length,
        GameLength::Hanchan,
        "半荘戦が length に反映されない"
    );
    assert_eq!(settings.initial_score, 35000);
    assert_eq!(setup.cpu_count(), 2);
}

#[test]
fn test_online_ui_build_rules_includes_selected_rules() {
    let mut online = OnlineUiState::new();
    assert!(online.build_rules().double_yakuman);

    online.rules.double_yakuman = false;
    online.rules.triple_ron_draw = true;
    online.rules.tsumo_loss = false;
    let rules = online.build_rules();
    assert!(!rules.double_yakuman);
    assert!(rules.triple_ron_draw);
    assert!(!rules.tsumo_loss);
}

#[test]
fn test_every_rule_option_toggles_its_user_facing_state() {
    let mut rules = mahjong_core::settings::Settings::new();
    for rule in RuleOption::ALL {
        let before = rule.is_enabled(&rules);
        rule.toggle(&mut rules);
        assert_ne!(rule.is_enabled(&rules), before, "{rule:?}");
        rule.toggle(&mut rules);
        assert_eq!(rule.is_enabled(&rules), before, "{rule:?}");
    }
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
        honba: 0,
        honba_points: 0,
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
            honba,
            honba_points,
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
            honba,
            honba_points,
            player_hands,
        },
        _ => unreachable!(),
    }
}

fn round_won_with_score(yaku_list: Vec<(ScoreItem, u32)>, han: u32) -> ServerEvent {
    ServerEvent::RoundWon {
        winner: Wind::East,
        loser: None,
        winning_tile: Tile::new(Tile::P1),
        scores: [25000; 4],
        yaku_list,
        han,
        fu: 30,
        score_points: 48000,
        rank: ScoreRank::Yakuman,
        has_opened: false,
        uradora_indicators: vec![],
        riichi_sticks: 0,
        honba: 0,
        honba_points: 0,
        player_hands: vec![],
    }
}

#[test]
fn test_counted_yakuman_result_uses_counted_label() {
    let mut state = GameState::new();
    state.lang = Lang::Ja;
    state.handle_event(game_started_4p(Wind::East, 0));

    state.handle_event(round_won_with_score(
        vec![
            (ScoreItem::Yaku(Kind::Riichi), 1),
            (ScoreItem::Dora(DoraLabel::Dora), 12),
        ],
        13,
    ));

    let result = state.current_win_result().expect("win result");
    assert_eq!(result.rank_name, "数え役満");
    assert_eq!(result.yakuman_multiplier, 0);
    assert!(
        result
            .result_message
            .contains("立直 1飜  ドラ 12飜\n数え役満 →")
    );
    assert!(!result.result_message.contains("30符"));
}

#[test]
fn test_yakuman_yaku_result_keeps_yakuman_label() {
    let mut state = GameState::new();
    state.lang = Lang::Ja;
    state.handle_event(game_started_4p(Wind::East, 0));

    state.handle_event(round_won_with_score(
        vec![(ScoreItem::Yaku(Kind::ThirteenOrphans), 13)],
        13,
    ));

    let result = state.current_win_result().expect("win result");
    assert_eq!(result.rank_name, "役満");
    assert_eq!(result.yakuman_multiplier, 1);
    assert!(result.result_message.contains("国士無双 役満 →"));
    assert!(!result.result_message.contains('飜'));
    assert!(!result.result_message.contains('符'));
}

#[test]
fn test_multiple_yakuman_result_uses_multiplier_without_han_or_fu() {
    let mut state = GameState::new();
    state.lang = Lang::Ja;
    state.handle_event(game_started_4p(Wind::East, 0));

    state.handle_event(round_won_with_score(
        vec![
            (ScoreItem::Yaku(Kind::FourConcealedTriplets), 13),
            (ScoreItem::Yaku(Kind::BigDragons), 13),
        ],
        26,
    ));

    let result = state.current_win_result().expect("win result");
    assert_eq!(result.rank_name, "二倍役満");
    assert_eq!(result.yakuman_multiplier, 2);
    assert!(result.result_message.contains("四暗刻  大三元 二倍役満 →"));
    assert!(!result.result_message.contains('飜'));
    assert!(!result.result_message.contains('符'));
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
fn test_round_won_separates_honba_and_deposits_from_hand_points() {
    let mut state = GameState::new();
    state.handle_event(game_started_4p(Wind::East, 0));

    let mut event = round_won_ron(Wind::South, Wind::East);
    let ServerEvent::RoundWon {
        score_points,
        riichi_sticks,
        honba,
        honba_points,
        ..
    } = &mut event
    else {
        unreachable!("helper always returns RoundWon")
    };
    // 5,200 hand points + 1,000 deposit points + two honba (600).
    *score_points = 6_800;
    *riichi_sticks = 1;
    *honba = 2;
    *honba_points = 600;

    state.handle_event(event);
    let result = state.current_win_result().expect("win result");
    assert_eq!(result.hand_points(), 5_200);
    assert_eq!(result.riichi_points(), 1_000);
    assert_eq!(result.honba, 2);
    assert_eq!(result.honba_points, 600);
}

#[test]
fn test_nagashi_mangan_uses_tileless_result_panel() {
    let mut state = GameState::new();
    state.lang = Lang::En;
    state.handle_event(game_started_4p(Wind::East, 0));
    state.riichi_sticks = 2;

    state.handle_event(ServerEvent::RoundNagashiMangan {
        winners: vec![
            mahjong_server::protocol::NagashiManganWinner {
                wind: Wind::South,
                score_points: 10_000,
            },
            mahjong_server::protocol::NagashiManganWinner {
                wind: Wind::West,
                score_points: 8_000,
            },
        ],
        scores: [9_000, 35_000, 33_000, 23_000],
        riichi_sticks: 2,
        player_hands: vec![PlayerHandInfo {
            wind: Wind::South,
            hand: vec![Tile::new(Tile::M1); 13],
            melds: vec![],
            pei: vec![],
        }],
    });

    assert_eq!(state.phase, GamePhase::RoundResult);
    assert_eq!(state.message_result_heading(), "Nagashi Mangan");
    assert!(state.current_win_result().is_none());
    assert!(state.win_tile.is_none());
    assert_eq!(state.scores, [9_000, 35_000, 33_000, 23_000]);
    assert_eq!(state.riichi_sticks, 0);
    assert!(state.other_players[0].revealed);
    let message = state.result_message.as_deref().expect("result message");
    assert!(message.contains("CPU1"));
    assert!(message.contains("10000 pts"));
    assert!(message.contains("CPU2"));
    assert!(message.contains("8000 pts"));
    assert!(message.contains("Deposits: 2"));
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
