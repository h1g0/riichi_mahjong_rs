//! Unit tests for `CpuClient`.

use super::*;
use crate::protocol::CallType;
use mahjong_core::tile::Wind;

fn game_started_event(seat_wind: Wind, hand: Vec<Tile>) -> ServerEvent {
    ServerEvent::GameStarted {
        seat_wind,
        hand,
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![],
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    }
}

/// Regression: the push/fold decision (should_attack) must count melds
/// as groups. Shanten used to be computed on a meld-less `Hand`, so open
/// hands looked "far" even at tenpai and always folded late in the hand.
#[test]
fn test_should_attack_counts_melds_when_tenpai() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // 123m/456m called; hand 789p + 11z + 34s is a 2s/5s tenpai.
    client.state.my_seat_wind = Wind::East;
    client.state.remaining_tiles = 10; // late in the hand (<=12 draws)
    client.state.my_hand = vec![
        Tile::new(Tile::P7),
        Tile::new(Tile::P8),
        Tile::new(Tile::P9),
        Tile::new(Tile::Z1),
        Tile::new(Tile::Z1),
        Tile::new(Tile::S3),
        Tile::new(Tile::S4),
    ];
    client.state.player_melds[0] = vec![
        Meld {
            tiles: vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: None,
        },
        Meld {
            tiles: vec![
                Tile::new(Tile::M4),
                Tile::new(Tile::M5),
                Tile::new(Tile::M6),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: None,
        },
    ];

    assert!(
        client.should_attack(),
        "副露込みで聴牌している手は終盤でも攻撃を続けるはず"
    );
}

// ===== North extraction (#257 Phase 3) =====

#[test]
fn test_consider_pei_declares_with_north_in_hand() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.three_player = true;
    client.state.nuki_dora = true;
    client.state.remaining_tiles = 30;
    // An ordinary hand holding a North (no orphan chase).
    client.state.my_hand = vec![
        Tile::new(Tile::P2),
        Tile::new(Tile::P3),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::P7),
        Tile::new(Tile::S2),
        Tile::new(Tile::S3),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::Z4),
    ];
    client.state.my_drawn = Some(Tile::new(Tile::S7));

    assert_eq!(client.consider_pei(), Some(ClientAction::Pei));
}

#[test]
fn test_consider_pei_none_in_four_player() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.three_player = false;
    client.state.my_hand = vec![Tile::new(Tile::Z4)];

    assert_eq!(client.consider_pei(), None);
}

#[test]
fn test_consider_pei_none_when_nuki_dora_disabled() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.three_player = true;
    client.state.nuki_dora = false;
    client.state.my_hand = vec![Tile::new(Tile::Z4)];

    assert_eq!(client.consider_pei(), None);
}

#[test]
fn test_consider_pei_keeps_north_for_kokushi() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.three_player = true;
    client.state.nuki_dora = true;
    // Thirteen Orphans 1-shanten; the North is needed.
    client.state.my_hand = vec![
        Tile::new(Tile::M1),
        Tile::new(Tile::M9),
        Tile::new(Tile::P1),
        Tile::new(Tile::P9),
        Tile::new(Tile::S1),
        Tile::new(Tile::S9),
        Tile::new(Tile::Z1),
        Tile::new(Tile::Z2),
        Tile::new(Tile::Z3),
        Tile::new(Tile::Z4),
        Tile::new(Tile::Z5),
        Tile::new(Tile::Z6),
        Tile::new(Tile::Z7),
    ];

    assert_eq!(client.consider_pei(), None);
}

#[test]
fn test_consider_pei_riichi_only_drawn_north() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.three_player = true;
    client.state.nuki_dora = true;
    client.state.is_riichi = true;
    client.state.remaining_tiles = 30;
    client.state.my_hand = vec![Tile::new(Tile::Z4)]; // hand tiles are locked under riichi

    assert_eq!(client.consider_pei(), None);

    client.state.my_drawn = Some(Tile::new(Tile::Z4));
    assert_eq!(client.consider_pei(), Some(ClientAction::Pei));
}

/// Regression (#296): never declare pei with the live wall empty
/// (post-haitei). The server rejects it for lack of a replacement draw,
/// and a rejected CPU is never re-consulted, so the hand stalled forever.
#[test]
fn test_consider_pei_none_when_wall_empty() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.three_player = true;
    client.state.nuki_dora = true;
    client.state.remaining_tiles = 0;
    client.state.my_hand = vec![Tile::new(Tile::P2), Tile::new(Tile::Z4)];
    client.state.my_drawn = Some(Tile::new(Tile::Z4));

    assert_eq!(client.consider_pei(), None);
}

/// Regression: a HandUpdated without our own call (the post-rejection
/// resync, #294) must not produce a discard. Replying would loop
/// reject -> resync -> re-discard -> reject and stall the whole hand,
/// including CPU substitution.
#[test]
fn test_resync_hand_updated_does_not_trigger_discard() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.state.my_seat_wind = Wind::South;

    let hand = vec![
        Tile::new(Tile::M1),
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::S7),
        Tile::new(Tile::S8),
        Tile::new(Tile::S9),
        Tile::new(Tile::Z1),
        Tile::new(Tile::Z1),
    ];

    // A resync HandUpdated (no own call): no discard.
    let action = client.handle_event(&ServerEvent::HandUpdated { hand: hand.clone() });
    assert_eq!(action, None, "再同期の HandUpdated に打牌を返している");

    // A HandUpdated following our pon still discards.
    client.handle_event(&ServerEvent::PlayerCalled {
        player: Wind::South,
        call_type: CallType::Pon,
        called_tile: Tile::new(Tile::Z5),
        tiles: vec![Tile::new(Tile::Z5); 3],
    });
    let action = client.handle_event(&ServerEvent::HandUpdated { hand });
    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "ポン直後の HandUpdated で打牌していない"
    );
}

#[test]
fn test_cpu_config_creation() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    assert_eq!(config.level, CpuLevel::Normal);
    assert_eq!(config.personality, CpuPersonality::Balanced);
}

#[test]
fn test_level_capabilities() {
    assert!(!CpuLevel::Weak.uses_acceptance_count());
    assert!(CpuLevel::Normal.uses_acceptance_count());
    assert!(CpuLevel::Strong.uses_acceptance_count());

    assert!(!CpuLevel::Weak.uses_value_estimation());
    assert!(!CpuLevel::Normal.uses_value_estimation());
    assert!(CpuLevel::Strong.uses_value_estimation());

    assert!(CpuLevel::Weak.should_make_mistake());
    assert!(!CpuLevel::Normal.should_make_mistake());
}

#[test]
fn test_level_ordering() {
    // The heuristics' level thresholds depend on this ordering.
    assert!(CpuLevel::Weak < CpuLevel::Normal);
    assert!(CpuLevel::Normal < CpuLevel::Strong);
}

#[test]
fn test_is_yakuhai() {
    assert!(is_yakuhai(Tile::Z5, Wind::East, Wind::East));
    assert!(is_yakuhai(Tile::Z6, Wind::East, Wind::East));
    assert!(is_yakuhai(Tile::Z7, Wind::East, Wind::East));
    assert!(is_yakuhai(Tile::Z1, Wind::East, Wind::East)); // round + seat wind
    assert!(!is_yakuhai(Tile::Z2, Wind::East, Wind::East)); // neither wind
}

#[test]
fn test_is_yakuhai_seat_and_prevailing_wind() {
    // South is a value honour for the South seat...
    assert!(is_yakuhai(Tile::Z2, Wind::South, Wind::East));
    // ...and in the South round...
    assert!(is_yakuhai(Tile::Z2, Wind::East, Wind::South));
    // ...but not otherwise.
    assert!(!is_yakuhai(Tile::Z2, Wind::East, Wind::East));
    // Dragons always are.
    assert!(is_yakuhai(Tile::Z5, Wind::North, Wind::West));
    assert!(is_yakuhai(Tile::Z6, Wind::North, Wind::West));
    assert!(is_yakuhai(Tile::Z7, Wind::North, Wind::West));
}

#[test]
fn test_is_tanyao_tile() {
    // Terminals and honours are not tanyao tiles.
    assert!(!is_tanyao_tile(Tile::M1));
    assert!(!is_tanyao_tile(Tile::M9));
    assert!(!is_tanyao_tile(Tile::P1));
    assert!(!is_tanyao_tile(Tile::P9));
    assert!(!is_tanyao_tile(Tile::S1));
    assert!(!is_tanyao_tile(Tile::S9));
    assert!(!is_tanyao_tile(Tile::Z1));
    assert!(!is_tanyao_tile(Tile::Z7));
    // Inside tiles are.
    assert!(is_tanyao_tile(Tile::M2));
    assert!(is_tanyao_tile(Tile::M8));
    assert!(is_tanyao_tile(Tile::P5));
    assert!(is_tanyao_tile(Tile::S7));
}

#[test]
fn test_tsumo_action() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
        ],
    ));

    let action = client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z2),
        remaining_tiles: 50,
        can_tsumo: true,
        can_riichi: false,
        is_furiten: false,
    });

    assert!(matches!(action, Some(ClientAction::Tsumo)));
}

#[test]
fn test_ron_action() {
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, vec![]));

    let action = client.handle_event(&ServerEvent::CallAvailable {
        tile: Tile::new(Tile::M1),
        discarder: Wind::East,
        calls: vec![AvailableCall::Ron],
    });

    assert!(matches!(action, Some(ClientAction::Ron)));
}

#[test]
fn test_discard_when_in_riichi_state() {
    // Under riichi with can_tsumo=false, tsumogiri.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
        ],
    ));
    client.handle_event(&ServerEvent::PlayerRiichi {
        player: Wind::East,
        scores: [24000, 25000, 25000, 25000],
        riichi_sticks: 1,
    });

    let action = client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::M5),
        remaining_tiles: 30,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });

    assert!(matches!(action, Some(ClientAction::Discard { tile: None })));
}

#[test]
fn test_riichi_action_when_can_riichi() {
    // can_riichi=true with enough aggressiveness returns Riichi.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // One tile short of tenpai (waiting on Z2).
    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
        ],
    ));

    // Drawing Z3 does not actually reach tenpai, but the server is
    // assumed to have set can_riichi.
    let action = client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z3),
        remaining_tiles: 30,
        can_tsumo: false,
        can_riichi: true,
        is_furiten: false,
    });

    assert!(matches!(action, Some(ClientAction::Riichi { .. })));
}

#[test]
fn test_riichi_with_ankan_melds_selects_tenpai_keeping_tile() {
    // Regression: the riichi declaration discard must be found in a hand
    // with concealed kans. Shanten used to ignore melds, so "no
    // tenpai-preserving discard" was misdetected and an invalid tsumogiri
    // riichi stalled the hand.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::P4),
            Tile::new(Tile::P4),
            Tile::new(Tile::P6),
            Tile::new(Tile::S1),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::S6),
        ],
    ));
    // Two concealed kans (M1, Z5) as melds.
    client.state.player_melds[0] = vec![
        Meld {
            tiles: vec![Tile::new(Tile::M1); 4],
            category: MeldType::Kan,
            from: MeldFrom::Myself,
            called_tile: None,
        },
        Meld {
            tiles: vec![Tile::new(Tile::Z5); 4],
            category: MeldType::Kan,
            from: MeldFrom::Myself,
            called_tile: None,
        },
    ];

    let action = client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::S5),
        remaining_tiles: 30,
        can_tsumo: false,
        can_riichi: true,
        is_furiten: false,
    });

    // Discarding P6 (keeping the S5S6 two-sided wait) is the only
    // tenpai-preserving riichi.
    assert!(
        matches!(
            action,
            Some(ClientAction::Riichi { tile: Some(t) }) if t.get() == Tile::P6
        ),
        "expected riichi discarding P6, got {action:?}"
    );
}

#[test]
fn test_riichi_falls_back_to_discard_when_no_tenpai_keeping_tile() {
    // Even with can_riichi set, no tenpai-preserving discard means a
    // normal discard: an invalid riichi would be rejected and stall.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // A scattered hand far from tenpai.
    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M4),
            Tile::new(Tile::M7),
            Tile::new(Tile::P2),
            Tile::new(Tile::P5),
            Tile::new(Tile::P8),
            Tile::new(Tile::S3),
            Tile::new(Tile::S6),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ],
    ));

    let action = client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z5),
        remaining_tiles: 30,
        can_tsumo: false,
        can_riichi: true,
        is_furiten: false,
    });

    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "expected fallback discard, got {action:?}"
    );
}

#[test]
fn test_discard_action_when_no_special_state() {
    // Neither tsumo nor riichi possible: Discard.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
            Tile::new(Tile::Z5),
        ],
    ));

    let action = client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z6),
        remaining_tiles: 30,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });

    assert!(matches!(action, Some(ClientAction::Discard { .. })));
}

fn draw_event(tile_type: u32) -> ServerEvent {
    ServerEvent::TileDrawn {
        tile: Tile::new(tile_type),
        remaining_tiles: 40,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    }
}

fn discarded_tile(action: &Option<ClientAction>) -> Option<Tile> {
    match action {
        Some(ClientAction::Discard { tile }) => *tile,
        _ => None,
    }
}

#[test]
fn test_discards_isolated_guest_wind_before_terminal() {
    // #147: among isolated tiles, guest winds go before terminals.
    // 3 groups + pair + three floaters (Z3 guest wind, P9, drawn S9).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::South,
        vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S4),
            Tile::new(Tile::S5),
            Tile::new(Tile::S6),
            Tile::new(Tile::M9),
            Tile::new(Tile::M9),
            Tile::new(Tile::P9),
            Tile::new(Tile::Z3),
        ],
    ));
    let action = client.handle_event(&draw_event(Tile::S9));

    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(tile.get(), Tile::Z3, "客風牌を最初に切るべき");
}

#[test]
fn test_discard_prefers_breaking_penchan_over_ryanmen() {
    // #148: with six blocks, edge shapes go before two-sided ones.
    // Blocks: M234 P456 M9M9 S6S7 (two-sided) P1P2 (edge) Z5Z5 (pair).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::South,
        vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::M9),
            Tile::new(Tile::M9),
            Tile::new(Tile::S6),
            Tile::new(Tile::S7),
            Tile::new(Tile::P1),
            Tile::new(Tile::P2),
            Tile::new(Tile::Z5),
        ],
    ));
    let action = client.handle_event(&draw_event(Tile::Z5));

    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert!(
        tile.get() == Tile::P1 || tile.get() == Tile::P2,
        "両面(S6S7)ではなく辺張(P1P2)を整理すべき, got {tile:?}"
    );
}

#[test]
fn test_dora_float_kept_over_plain_float() {
    // #152: between equal floaters (isolated terminals), keep the dora.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::M9),
        Tile::new(Tile::M9),
        Tile::new(Tile::M9),
        Tile::new(Tile::P9),
    ];
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand: hand.clone(),
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P8)], // dora is P9
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    });
    // Four groups plus a pair-wait choice between P9 (dora) and the
    // drawn S9: keep the dora, tsumogiri the S9.
    let action = client.handle_event(&draw_event(Tile::S9));
    assert!(
        matches!(action, Some(ClientAction::Discard { tile: None })),
        "ドラ(P9)を残して S9 をツモ切りすべき, got {action:?}"
    );

    // Control: with heuristics off P9 goes (no dora protection;
    // the first equal-scored candidate wins).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand,
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P8)],
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    });
    let action = client.handle_event(&draw_event(Tile::S9));
    assert!(
        matches!(action, Some(ClientAction::Discard { tile: Some(t) }) if t.get() == Tile::P9),
        "定石無効時はドラ保護が効かない, got {action:?}"
    );
}

#[test]
fn test_weak_folds_with_genbutsu_against_riichi() {
    // #173/#174: even Weak folds to a riichi starting from genbutsu,
    // even when the genbutsu is part of a pair - safety beats closeness.
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::S1),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z3),
            Tile::new(Tile::P2),
            Tile::new(Tile::P5),
            Tile::new(Tile::P9),
            Tile::new(Tile::S9),
            Tile::new(Tile::M9),
        ],
    ));
    // South discarded Z3 before declaring riichi.
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::Z3),
        is_tsumogiri: false,
        hand_index: None,
    });
    client.handle_event(&ServerEvent::PlayerRiichi {
        player: Wind::South,
        scores: [25000, 24000, 25000, 25000],
        riichi_sticks: 1,
    });
    let action = client.handle_event(&draw_event(Tile::M5));

    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(tile.get(), Tile::Z3, "現物(Z3)を最優先で切るべき");
}

#[test]
fn test_defense_prefers_suji_over_dangerous_tiles() {
    // #176: without genbutsu, suji/honour-ish tiles beat off-suji inside tiles.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::S1),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z3),
            Tile::new(Tile::M7),
            Tile::new(Tile::P9),
            Tile::new(Tile::S9),
            Tile::new(Tile::S6),
            Tile::new(Tile::P2),
        ],
    ));
    // South discarded M4 before riichi, making M7 suji.
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::M4),
        is_tsumogiri: false,
        hand_index: None,
    });
    client.handle_event(&ServerEvent::PlayerRiichi {
        player: Wind::South,
        scores: [25000, 24000, 25000, 25000],
        riichi_sticks: 1,
    });
    let action = client.handle_event(&draw_event(Tile::P5));

    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(tile.get(), Tile::M7, "筋牌(M7)を選ぶべき, got {tile:?}");
}

#[test]
fn test_riichi_declared_with_no_yaku_tenpai() {
    // #168: a yakuless tenpai declares even where the legacy judgement
    // would not. Speedy (aggressiveness 0.4) normally stays quiet against
    // two riichi, but a yakuless damaten cannot win at all, so the
    // heuristic forces the declaration.
    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::M7),
        Tile::new(Tile::M9),
        Tile::new(Tile::Z3),
        Tile::new(Tile::Z3),
    ];
    let riichi = |player| ServerEvent::PlayerRiichi {
        player,
        scores: [25000; 4],
        riichi_sticks: 1,
    };
    let draw = ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z4),
        remaining_tiles: 40,
        can_tsumo: false,
        can_riichi: true,
        is_furiten: false,
    };

    // Heuristics on: declare.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    client.handle_event(&riichi(Wind::South));
    client.handle_event(&riichi(Wind::West));
    let action = client.handle_event(&draw);
    assert!(
        matches!(action, Some(ClientAction::Riichi { .. })),
        "役なし聴牌はリーチすべき, got {action:?}"
    );

    // Heuristics off: legacy stays quiet against two riichi.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    client.handle_event(&riichi(Wind::South));
    client.handle_event(&riichi(Wind::West));
    let action = client.handle_event(&draw);
    assert!(matches!(action, Some(ClientAction::Discard { .. })));
}

#[test]
fn test_damaten_with_confirmed_mangan() {
    // #170: mangan even damaten (tanyao+pinfu+3 dora): no riichi.
    let hand = vec![
        Tile::new(Tile::P2),
        Tile::new(Tile::P3),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::P7),
        Tile::new(Tile::S3),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S8),
        Tile::new(Tile::S8),
        Tile::new(Tile::M4),
        Tile::new(Tile::M5),
    ];
    let start = |hand: Vec<Tile>| ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand,
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::S7), Tile::new(Tile::M3)], // dora: two S8 + M4
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    };
    let draw = ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z3),
        remaining_tiles: 40,
        can_tsumo: false,
        can_riichi: true,
        is_furiten: false,
    };

    // Heuristics on: damaten (a normal discard).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&start(hand.clone()));
    let action = client.handle_event(&draw);
    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "満貫確定はダマにすべき, got {action:?}"
    );

    // Heuristics off: the aggressiveness judgement declares.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&start(hand));
    let action = client.handle_event(&draw);
    assert!(matches!(action, Some(ClientAction::Riichi { .. })));
}

#[test]
fn test_cheap_bad_shape_tenpai_folds_against_riichi() {
    // #178: a bad-shape cheap tenpai (tanyao only, closed wait) folds to
    // a riichi from genbutsu (even out of a pair) instead of pushing;
    // tenpai used to push unconditionally.
    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::M6),
        Tile::new(Tile::M8),
        Tile::new(Tile::S2),
        Tile::new(Tile::S2),
    ];
    let riichi_with_genbutsu = |client: &mut CpuClient| {
        client.handle_event(&ServerEvent::TileDiscarded {
            player: Wind::West,
            tile: Tile::new(Tile::S2),
            is_tsumogiri: false,
            hand_index: None,
        });
        client.handle_event(&ServerEvent::PlayerRiichi {
            player: Wind::West,
            scores: [25000, 25000, 24000, 25000],
            riichi_sticks: 1,
        });
    };

    // Heuristics on: fold with the genbutsu S2, breaking tenpai.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand.clone()));
    riichi_with_genbutsu(&mut client);
    let action = client.handle_event(&draw_event(Tile::Z4));
    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(tile.get(), Tile::S2, "愚形安手聴牌は現物から降りるべき");

    // Heuristics off: keep tenpai and push (tsumogiri).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand));
    riichi_with_genbutsu(&mut client);
    let action = client.handle_event(&draw_event(Tile::Z4));
    assert!(
        matches!(action, Some(ClientAction::Discard { tile: None })),
        "定石無効時は聴牌維持（ツモ切り）, got {action:?}"
    );
}

#[test]
fn test_folds_against_three_meld_opponent() {
    // #180: an opponent with three melds counts as likely tenpai even
    // without riichi; a far hand folds from genbutsu.
    let hand = vec![
        Tile::new(Tile::M1),
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::S1),
        Tile::new(Tile::S2),
        Tile::new(Tile::S3),
        Tile::new(Tile::Z3),
        Tile::new(Tile::Z3),
        Tile::new(Tile::P2),
        Tile::new(Tile::P5),
        Tile::new(Tile::P9),
        Tile::new(Tile::S9),
        Tile::new(Tile::M9),
    ];
    let melds = vec![
        Meld {
            tiles: vec![
                Tile::new(Tile::M4),
                Tile::new(Tile::M5),
                Tile::new(Tile::M6),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(Tile::new(Tile::M4)),
        },
        Meld {
            tiles: vec![Tile::new(Tile::P7); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::P7)),
        },
        Meld {
            tiles: vec![Tile::new(Tile::S6); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::S6)),
        },
    ];

    // South has three melds and has discarded Z3.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    client.state.player_melds[1] = melds.clone();
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::Z3),
        is_tsumogiri: false,
        hand_index: None,
    });
    let action = client.handle_event(&draw_event(Tile::M5));
    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(
        tile.get(),
        Tile::Z3,
        "3副露の他家に対して現物(Z3)からベタオリすべき"
    );

    // Heuristics off: melds are not a threat; normal discard.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    client.state.player_melds[1] = melds;
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::Z3),
        is_tsumogiri: false,
        hand_index: None,
    });
    let action = client.handle_event(&draw_event(Tile::M5));
    if let Some(t) = discarded_tile(&action) {
        assert_ne!(t.get(), Tile::Z3, "定石無効時は対子の現物を崩さない");
    }
}

#[test]
fn test_six_block_hand_dismantles_dead_kanchan_first() {
    // #149/#151/#153 together: in a six-block hand
    // (M234 S789 Z5Z5 P1P2 S2S4 P78), the S2S4 closed shape killed by
    // three visible S3 goes first; the two-sided P78 and the only pair
    // Z5Z5 are protected.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::South,
        vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z5),
            Tile::new(Tile::Z5),
            Tile::new(Tile::P1),
            Tile::new(Tile::P2),
            Tile::new(Tile::S2),
            Tile::new(Tile::S4),
            Tile::new(Tile::P7),
        ],
    ));
    // Three S3 on the table kill the S2S4 shape.
    for _ in 0..3 {
        client.handle_event(&ServerEvent::TileDiscarded {
            player: Wind::West,
            tile: Tile::new(Tile::S3),
            is_tsumogiri: true,
            hand_index: None,
        });
    }
    let action = client.handle_event(&draw_event(Tile::P8));

    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert!(
        tile.get() == Tile::S2 || tile.get() == Tile::S4,
        "死に嵌張(S2S4)を整理すべき, got {tile:?}"
    );
}

/// Builds a 14-tile deal with n orphan kinds padded with inside tiles.
fn orphan_rich_hand(kinds: usize) -> Vec<Tile> {
    let orphan_types = [
        Tile::M1,
        Tile::M9,
        Tile::P1,
        Tile::P9,
        Tile::S1,
        Tile::S9,
        Tile::Z1,
        Tile::Z2,
        Tile::Z3,
        Tile::Z4,
        Tile::Z5,
        Tile::Z6,
        Tile::Z7,
    ];
    let fillers = [Tile::M4, Tile::P5, Tile::S6, Tile::M6, Tile::P3];
    let mut hand: Vec<Tile> = orphan_types
        .iter()
        .take(kinds)
        .map(|&t| Tile::new(t))
        .collect();
    hand.extend(fillers.iter().take(13 - kinds).map(|&t| Tile::new(t)));
    hand
}

fn nine_terminals_action(
    config: CpuConfig,
    hand: Vec<Tile>,
    scores: [i32; 4],
) -> Option<ClientAction> {
    let mut client = CpuClient::new(config);
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::East,
        hand,
        scores,
        round_wind: Wind::East,
        dora_indicators: vec![],
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    });
    client.handle_event(&ServerEvent::TileDrawn {
        tile: Tile::new(Tile::S5),
        remaining_tiles: 69,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    });
    client.handle_event(&ServerEvent::NineTerminalsAvailable)
}

#[test]
fn test_kokushi_hand_keeps_orphans() {
    // #160: with ten orphan kinds, keep the orphans and discard inside
    // tiles. Without the route lock the standard-form shanten would pull
    // an orphan discard.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::East, orphan_rich_hand(10)));
    let action = client.handle_event(&draw_event(Tile::P6));

    let tile = discarded_tile(&action);
    // Tsumogiri (P6) or any inside-tile discard is correct.
    if let Some(t) = tile {
        assert!(
            !t.is_1_9_honour(),
            "国士無双ルートでは么九牌を切らない, got {t:?}"
        );
    }
}

#[test]
fn test_nine_terminals_continues_with_ten_kinds() {
    // #160: ten orphan kinds continue regardless of personality.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let action = nine_terminals_action(config, orphan_rich_hand(10), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));
}

#[test]
fn test_nine_terminals_nine_kinds_depends_on_situation() {
    // 9 kinds: a flat-score Balanced declares the draw.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let action = nine_terminals_action(config, orphan_rich_hand(9), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: true })
    ));

    // 9 kinds: HighValue continues for the orphans.
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue);
    let action = nine_terminals_action(config, orphan_rich_hand(9), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));

    // 9 kinds: far behind (#159) even Balanced continues.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let action = nine_terminals_action(config, orphan_rich_hand(9), [8000, 42000, 25000, 25000]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));
}

#[test]
fn test_nine_terminals_without_heuristics_uses_personality() {
    // Heuristics off: legacy, only HighValue continues.
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue).without_heuristics();
    let action = nine_terminals_action(config, orphan_rich_hand(9), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));

    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let action = nine_terminals_action(config, orphan_rich_hand(10), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: true })
    ));
}

#[test]
fn test_handle_event_returns_none_for_non_actionable() {
    // Discards, other players' draws etc. require no action.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    let events = [
        ServerEvent::TileDiscarded {
            player: Wind::South,
            tile: Tile::new(Tile::M1),
            is_tsumogiri: false,
            hand_index: None,
        },
        ServerEvent::OtherPlayerDrew {
            player: Wind::South,
            remaining_tiles: 50,
        },
        ServerEvent::PlayerRiichi {
            player: Wind::South,
            scores: [25000; 4],
            riichi_sticks: 1,
        },
    ];

    for event in &events {
        assert!(
            client.handle_event(event).is_none(),
            "expected None for {event:?}"
        );
    }
}

#[test]
fn test_pass_when_chi_only_and_high_value() {
    // HighValue never calls chii: Pass.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::HighValue);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(
        Wind::South,
        vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z2),
        ],
    ));

    let action = client.handle_event(&ServerEvent::CallAvailable {
        tile: Tile::new(Tile::M1),
        discarder: Wind::East,
        calls: vec![AvailableCall::Chi {
            options: vec![[Tile::new(Tile::M2), Tile::new(Tile::M3)]],
        }],
    });

    assert!(matches!(action, Some(ClientAction::Pass)));
}

#[test]
fn test_pon_yakuhai_normal_level() {
    // A value-honour pon that lowers shanten is taken even at Normal.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // 1-shanten with two Z5: M123+P456+S789 complete, Z5Z5 head,
    // Z2Z3 floaters; the Z5 pon drops 1 -> 0.
    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::Z5),
            Tile::new(Tile::Z5),
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
        ],
    ));

    let action = client.handle_event(&ServerEvent::CallAvailable {
        tile: Tile::new(Tile::Z5),
        discarder: Wind::South,
        calls: vec![AvailableCall::Pon {
            options: vec![[Tile::new(Tile::Z5), Tile::new(Tile::Z5)]],
        }],
    });

    assert!(matches!(action, Some(ClientAction::Pon { .. })));
}

#[test]
fn test_pon_not_called_when_shanten_does_not_decrease() {
    // A pon that does not lower shanten is passed. At a Thirteen Orphans
    // tenpai (13 orphans + pair) the shanten is 0, but a Z5 pon opens the
    // hand and the shanten jumps.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // 12 orphan kinds + two Z5: an orphans tenpai that a pon would ruin.
    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M9),
            Tile::new(Tile::P1),
            Tile::new(Tile::P9),
            Tile::new(Tile::S1),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
            Tile::new(Tile::Z5),
            Tile::new(Tile::Z5),
            Tile::new(Tile::Z7),
        ],
    ));

    let action = client.handle_event(&ServerEvent::CallAvailable {
        tile: Tile::new(Tile::Z5),
        discarder: Wind::South,
        calls: vec![AvailableCall::Pon {
            options: vec![[Tile::new(Tile::Z5), Tile::new(Tile::Z5)]],
        }],
    });

    assert!(matches!(action, Some(ClientAction::Pass)));
}

/// Shared hand offering a yakuless call: an M9 pon lowers the shanten
/// but leaves no yaku. The floaters are guest winds (Z3/Z4, not value
/// honours for South) to prevent accidental tenpai via suit
/// re-decomposition.
fn yakuless_pon_hand() -> Vec<Tile> {
    vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::P3),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::M9),
        Tile::new(Tile::M9),
        Tile::new(Tile::Z3),
        Tile::new(Tile::Z4),
    ]
}

fn pon_call_event(tile_type: u32) -> ServerEvent {
    ServerEvent::CallAvailable {
        tile: Tile::new(tile_type),
        discarder: Wind::East,
        calls: vec![AvailableCall::Pon {
            options: vec![[Tile::new(tile_type), Tile::new(tile_type)]],
        }],
    }
}

#[test]
fn test_pass_on_yakuless_pon() {
    // #162: a call with no yaku prospect is declined even if it
    // lowers the shanten.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, yakuless_pon_hand()));
    let action = client.handle_event(&pon_call_event(Tile::M9));

    assert!(matches!(action, Some(ClientAction::Pass)));
}

#[test]
fn test_yakuless_pon_called_without_heuristics() {
    // Heuristics off: legacy calls (the A/B baseline).
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, yakuless_pon_hand()));
    let action = client.handle_event(&pon_call_event(Tile::M9));

    assert!(matches!(action, Some(ClientAction::Pon { .. })));
}

#[test]
fn test_weak_level_also_avoids_yakuless_pon() {
    // #162 is weak+: even Weak declines yakuless calls.
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, yakuless_pon_hand()));
    let action = client.handle_event(&pon_call_event(Tile::M9));

    assert!(matches!(action, Some(ClientAction::Pass)));
}

#[test]
fn test_high_value_pons_yakuhai() {
    // #163: value-honour pair pons ignore call aggressiveness.
    let hand = vec![
        Tile::new(Tile::Z5),
        Tile::new(Tile::Z5),
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::P6),
        Tile::new(Tile::S2),
        Tile::new(Tile::S2),
        Tile::new(Tile::M7),
        Tile::new(Tile::M8),
        Tile::new(Tile::S9),
    ];

    // HighValue (aggressiveness 0.2) used to decline even value honours.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::HighValue);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand.clone()));
    let action = client.handle_event(&pon_call_event(Tile::Z5));
    assert!(matches!(action, Some(ClientAction::Pon { .. })));

    // Heuristics off: legacy passes.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::HighValue).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand));
    let action = client.handle_event(&pon_call_event(Tile::Z5));
    assert!(matches!(action, Some(ClientAction::Pass)));
}

fn chi_call_event(tile_type: u32, hand_tiles: [u32; 2]) -> ServerEvent {
    ServerEvent::CallAvailable {
        tile: Tile::new(tile_type),
        discarder: Wind::East,
        calls: vec![AvailableCall::Chi {
            options: vec![[Tile::new(hand_tiles[0]), Tile::new(hand_tiles[1])]],
        }],
    }
}

#[test]
fn test_kuitan_chi_requires_simple_centered_hand() {
    // #164 (normal+): no kuitan chii from a hand still holding
    // three orphans.
    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M9),
        Tile::new(Tile::P9),
        Tile::new(Tile::S9),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::S6),
        Tile::new(Tile::S7),
        Tile::new(Tile::P2),
        Tile::new(Tile::S2),
        Tile::new(Tile::M6),
        Tile::new(Tile::P7),
    ];

    // Normal: no prospect under the strict rule - pass.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand.clone()));
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // Weak: the loose rule (<=3 orphans) still calls.
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand));
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Chi { .. })));
}

#[test]
fn test_cheap_distant_chi_suppressed() {
    // #165: a non-dealer avoids value-less calls above 2-shanten.
    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::P4),
        Tile::new(Tile::P5),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::P7),
        Tile::new(Tile::P8),
        Tile::new(Tile::S2),
        Tile::new(Tile::S3),
        Tile::new(Tile::M7),
        Tile::new(Tile::M8),
        Tile::new(Tile::S8),
    ];

    // No dora: cheap and distant - pass.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand: hand.clone(),
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::Z5)], // the dora (Green) is not in hand
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    });
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // Two dora give a value prospect: call.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand,
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P6)], // dora is P7 (held twice)
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
        three_player: false,
        nuki_dora: false,
    });
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Chi { .. })));
}

#[test]
fn test_toitoi_pon_requires_four_blocks() {
    // #157: toitoi pons need melds + pairs/triplets >= 4 blocks.
    let s9_pon = Meld {
        tiles: vec![Tile::new(Tile::S9); 3],
        category: MeldType::Pon,
        from: MeldFrom::Unknown,
        called_tile: Some(Tile::new(Tile::S9)),
    };

    // Three blocks (1 meld + M9M9 + P1P1): the M9 pon has no prospect - pass.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M9),
            Tile::new(Tile::M9),
            Tile::new(Tile::P1),
            Tile::new(Tile::P1),
            Tile::new(Tile::P4),
            Tile::new(Tile::M2),
            Tile::new(Tile::S3),
            Tile::new(Tile::M6),
            Tile::new(Tile::P7),
            Tile::new(Tile::S5),
        ],
    ));
    client.state.player_melds[0] = vec![s9_pon.clone()];
    let action = client.handle_event(&pon_call_event(Tile::M9));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // Five blocks (1 meld + 4 pairs): toitoi prospect - call.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(
        Wind::East,
        vec![
            Tile::new(Tile::M9),
            Tile::new(Tile::M9),
            Tile::new(Tile::P1),
            Tile::new(Tile::P1),
            Tile::new(Tile::S3),
            Tile::new(Tile::S3),
            Tile::new(Tile::P6),
            Tile::new(Tile::P6),
            Tile::new(Tile::M2),
            Tile::new(Tile::S5),
        ],
    ));
    client.state.player_melds[0] = vec![s9_pon];
    let action = client.handle_event(&pon_call_event(Tile::M9));
    assert!(matches!(action, Some(ClientAction::Pon { .. })));
}

#[test]
fn test_pass_on_pon_leading_to_naked_tanki() {
    // #166: never pon into a fourth meld (bare pair).
    let hand = vec![
        Tile::new(Tile::S3),
        Tile::new(Tile::S3),
        Tile::new(Tile::M5),
        Tile::new(Tile::M9),
    ];
    let melds = vec![
        Meld {
            tiles: vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(Tile::new(Tile::M1)),
        },
        Meld {
            tiles: vec![Tile::new(Tile::P5); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::P5)),
        },
        Meld {
            tiles: vec![Tile::new(Tile::S9); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::S9)),
        },
    ];

    // Heuristics on: pass.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    client.state.player_melds[0] = melds.clone();
    let action = client.handle_event(&pon_call_event(Tile::S3));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // Heuristics off: legacy calls.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    client.state.player_melds[0] = melds;
    let action = client.handle_event(&pon_call_event(Tile::S3));
    assert!(matches!(action, Some(ClientAction::Pon { .. })));
}

#[test]
fn test_normal_level_avoids_hand_breaking_ankan() {
    // #167 (normal+): never kan when it breaks the hand.
    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::P2),
        Tile::new(Tile::P3),
        Tile::new(Tile::P4),
        Tile::new(Tile::S4),
        Tile::new(Tile::S5),
        Tile::new(Tile::S5),
        Tile::new(Tile::S5),
        Tile::new(Tile::S6),
        Tile::new(Tile::Z1),
        Tile::new(Tile::Z3),
    ];
    let draw_event = ServerEvent::TileDrawn {
        tile: Tile::new(Tile::S5),
        remaining_tiles: 40,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    };

    // Heuristics on: the four S5 serve S456+S555, so discard instead.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    let action = client.handle_event(&draw_event);
    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "expected discard instead of hand-breaking kan, got {action:?}"
    );

    // Heuristics off: legacy Normal kans anyway.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    let action = client.handle_event(&draw_event);
    assert!(matches!(action, Some(ClientAction::Kan { .. })));
}

#[test]
fn test_ankan_suppressed_during_opponent_riichi() {
    // #167: under a riichi, only a tenpai-preserving kan is made.
    let hand = vec![
        Tile::new(Tile::M2),
        Tile::new(Tile::M3),
        Tile::new(Tile::M4),
        Tile::new(Tile::M6),
        Tile::new(Tile::M7),
        Tile::new(Tile::S3),
        Tile::new(Tile::S3),
        Tile::new(Tile::P2),
        Tile::new(Tile::P2),
        Tile::new(Tile::P2),
        Tile::new(Tile::P2),
        Tile::new(Tile::Z1),
        Tile::new(Tile::Z2),
    ];
    let draw_event = ServerEvent::TileDrawn {
        tile: Tile::new(Tile::M5),
        remaining_tiles: 40,
        can_tsumo: false,
        can_riichi: false,
        is_furiten: false,
    };

    // No riichi: the shanten-preserving kan happens.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    let action = client.handle_event(&draw_event);
    assert!(matches!(action, Some(ClientAction::Kan { .. })));

    // With a riichi and no tenpai after: no kan.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    client.handle_event(&ServerEvent::PlayerRiichi {
        player: Wind::West,
        scores: [25000, 25000, 24000, 25000],
        riichi_sticks: 1,
    });
    let action = client.handle_event(&draw_event);
    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "expected discard instead of kan during opponent riichi, got {action:?}"
    );
}

#[test]
fn test_pass_when_daiminkan_only_non_strong_high_value() {
    // Called quads: everyone but Strong+HighValue passes.
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, vec![]));

    let action = client.handle_event(&ServerEvent::CallAvailable {
        tile: Tile::new(Tile::M1),
        discarder: Wind::East,
        calls: vec![AvailableCall::Daiminkan],
    });

    assert!(matches!(action, Some(ClientAction::Pass)));
}

/// shuffle_cpu_configs must permute without altering the configs.
#[test]
fn test_shuffle_cpu_configs_preserves_configs() {
    let original = [
        CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue),
    ];

    let mut configs = original.clone();
    shuffle_cpu_configs(&mut configs);

    // The multiset of configs is unchanged.
    for c in &original {
        assert!(
            configs
                .iter()
                .any(|s| s.level == c.level && s.personality == c.personality),
            "シャッフルで設定が失われた"
        );
    }
}

/// Configs outside the slice (non-playing CPUs) must be untouched.
#[test]
fn test_shuffle_cpu_configs_respects_slice_bounds() {
    for _ in 0..20 {
        let mut configs = [
            CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced),
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy),
            CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue),
        ];
        // Three-player style: shuffle only the first two.
        shuffle_cpu_configs(&mut configs[..2]);
        assert_eq!(configs[2].level, CpuLevel::Strong);
        assert_eq!(configs[2].personality, CpuPersonality::HighValue);
    }
}
