//! `CpuClient` のユニットテスト

use super::*;
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
    }
}

/// 回帰テスト: 押し引き判断（should_attack）が副露を面子として数えること
///
/// かつて副露なしの `Hand` で向聴数を計算していたため、鳴いた手は
/// 聴牌していても「遠い手」と誤判定され、終盤に必ず降りていた。
#[test]
fn test_should_attack_counts_melds_when_tenpai() {
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // 123m・456m をチー済み。残り手牌 789p + 11z + 34s（2s/5s 待ちの聴牌）
    client.state.my_seat_wind = Wind::East;
    client.state.remaining_tiles = 10; // 終盤（残りツモ12枚以下）
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
    // 定石の「弱以上」「中以上」判定はこの順序に依存する
    assert!(CpuLevel::Weak < CpuLevel::Normal);
    assert!(CpuLevel::Normal < CpuLevel::Strong);
}

#[test]
fn test_is_yakuhai() {
    assert!(is_yakuhai(Tile::Z5, Wind::East, Wind::East)); // 白
    assert!(is_yakuhai(Tile::Z6, Wind::East, Wind::East)); // 發
    assert!(is_yakuhai(Tile::Z7, Wind::East, Wind::East)); // 中
    assert!(is_yakuhai(Tile::Z1, Wind::East, Wind::East)); // 東（場風+自風）
    assert!(!is_yakuhai(Tile::Z2, Wind::East, Wind::East)); // 南（場風でも自風でもない）
}

#[test]
fn test_is_yakuhai_seat_and_prevailing_wind() {
    // 自風が南のとき、Z2（南）は役牌
    assert!(is_yakuhai(Tile::Z2, Wind::South, Wind::East));
    // 場風が南のとき、Z2（南）は役牌
    assert!(is_yakuhai(Tile::Z2, Wind::East, Wind::South));
    // どちらでもないとき、Z2 は役牌でない
    assert!(!is_yakuhai(Tile::Z2, Wind::East, Wind::East));
    // 三元牌は常に役牌
    assert!(is_yakuhai(Tile::Z5, Wind::North, Wind::West));
    assert!(is_yakuhai(Tile::Z6, Wind::North, Wind::West));
    assert!(is_yakuhai(Tile::Z7, Wind::North, Wind::West));
}

#[test]
fn test_is_tanyao_tile() {
    // 端牌・字牌は非タンヤオ
    assert!(!is_tanyao_tile(Tile::M1));
    assert!(!is_tanyao_tile(Tile::M9));
    assert!(!is_tanyao_tile(Tile::P1));
    assert!(!is_tanyao_tile(Tile::P9));
    assert!(!is_tanyao_tile(Tile::S1));
    assert!(!is_tanyao_tile(Tile::S9));
    assert!(!is_tanyao_tile(Tile::Z1));
    assert!(!is_tanyao_tile(Tile::Z7));
    // 中張牌はタンヤオ
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
    // リーチ中はcan_tsumo=falseのときツモ切りを返す
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
    // can_riichi=true かつリーチ積極度が十分なら Riichi を返す
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // テンパイ1枚前の手牌（Z2待ち）
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

    // Z3をツモ → Z2Z3の順子形成でもテンパイにならないが、
    // can_riichi フラグをサーバが立てている想定
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
    // 暗カンを含む手牌でもリーチ宣言牌を正しく選べる（回帰テスト）。
    // 以前は副露を無視して向聴数を計算していたため「聴牌維持牌なし」と
    // 誤判定し、不正なツモ切りリーチを送信して局が進行不能になっていた。
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
    // 暗カン2つ（M1, Z5）を副露情報としてセット
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

    // P6切りリーチ（S5S6の両面を残す）が唯一の聴牌維持打牌
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
    // can_riichi が立っていても聴牌維持牌が見つからなければ
    // リーチせず通常打牌に進む（不正なリーチはサーバに拒否され停滞する）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // 大きく聴牌から遠いバラバラの手牌
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
    // ツモ和了不可・リーチ不可のとき Discard を返す
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
    // #147: 孤立牌の中でも客風牌を1・9牌より先に切る
    // 3面子 + 雀頭 + 浮き牌3枚（Z3=客風, P9, ツモS9）
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
    // #148: 6ブロックの手では両面より辺張を整理する
    // ブロック: M234 P456 M9M9 S6S7(両面) P1P2(辺張) Z5Z5(ツモで対子)
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
    // #152: 同価値の浮き牌（孤立1・9牌）ならドラでない方を切る
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
        dora_indicators: vec![Tile::new(Tile::P8)], // ドラは P9
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
    });
    // 4面子完成 + P9(ドラ) + ツモ S9 の単騎選択。
    // ドラの P9 を残して S9 をツモ切りすべき
    let action = client.handle_event(&draw_event(Tile::S9));
    assert!(
        matches!(action, Some(ClientAction::Discard { tile: None })),
        "ドラ(P9)を残して S9 をツモ切りすべき, got {action:?}"
    );

    // 対照: 定石無効なら P9 を切る（ドラ保護なし、同値で先頭の候補が選ばれる）
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
    });
    let action = client.handle_event(&draw_event(Tile::S9));
    assert!(
        matches!(action, Some(ClientAction::Discard { tile: Some(t) }) if t.get() == Tile::P9),
        "定石無効時はドラ保護が効かない, got {action:?}"
    );
}

#[test]
fn test_weak_folds_with_genbutsu_against_riichi() {
    // #173/#174: 弱レベルでも他家リーチに対して現物からベタオリする
    // （現物が対子の一部でも、聴牌への近さより安全を優先する）
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
    // 南家が Z3 を切ってからリーチ
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::Z3),
        is_tsumogiri: false,
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
    // #176: 現物がない場合、無筋の中張牌より筋・字牌寄りの牌を選ぶ
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
    // 南家が M4 を切ってからリーチ → M7 は筋
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::M4),
        is_tsumogiri: false,
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
    // #168: 役なし聴牌は（従来なら宣言を控える局面でも）リーチする。
    // Speedy（リーチ積極度0.4）は2人リーチに対して従来は宣言しないが、
    // 役なしダマは和了できないため定石が宣言を強制する。
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

    // 定石有効: リーチ宣言
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

    // 定石無効: 従来どおり2人リーチには宣言しない
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
    // #170: ダマでも満貫（タンヤオ+ピンフ+ドラ3）ならリーチしない
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
        dora_indicators: vec![Tile::new(Tile::S7), Tile::new(Tile::M3)], // ドラ S8×2 + M4
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
    };
    let draw = ServerEvent::TileDrawn {
        tile: Tile::new(Tile::Z3),
        remaining_tiles: 40,
        can_tsumo: false,
        can_riichi: true,
        is_furiten: false,
    };

    // 定石有効: ダマ（通常打牌）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&start(hand.clone()));
    let action = client.handle_event(&draw);
    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "満貫確定はダマにすべき, got {action:?}"
    );

    // 定石無効: 従来の積極度判断でリーチする
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&start(hand));
    let action = client.handle_event(&draw);
    assert!(matches!(action, Some(ClientAction::Riichi { .. })));
}

#[test]
fn test_cheap_bad_shape_tenpai_folds_against_riichi() {
    // #178: 愚形安手（タンヤオのみ・カンチャン待ち）の聴牌はリーチに押さず、
    // 現物（対子の一部でも）から降りる。従来は聴牌なら無条件に押していた。
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
        });
        client.handle_event(&ServerEvent::PlayerRiichi {
            player: Wind::West,
            scores: [25000, 25000, 24000, 25000],
            riichi_sticks: 1,
        });
    };

    // 定石有効: 降りて現物(S2)を切る（聴牌は崩れる）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand.clone()));
    riichi_with_genbutsu(&mut client);
    let action = client.handle_event(&draw_event(Tile::Z4));
    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(tile.get(), Tile::S2, "愚形安手聴牌は現物から降りるべき");

    // 定石無効: 従来どおり聴牌を維持して押す（ツモ切り）
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
    // #180: リーチがなくても3副露の他家は聴牌濃厚として扱い、
    // 遠い手なら現物からベタオリする
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

    // 南家が3副露 + Z3 を捨てている
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    client.state.player_melds[1] = melds.clone();
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::Z3),
        is_tsumogiri: false,
    });
    let action = client.handle_event(&draw_event(Tile::M5));
    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert_eq!(
        tile.get(),
        Tile::Z3,
        "3副露の他家に対して現物(Z3)からベタオリすべき"
    );

    // 定石無効: 副露者は脅威とみなさず通常打牌（現物優先にならない）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    client.state.player_melds[1] = melds;
    client.handle_event(&ServerEvent::TileDiscarded {
        player: Wind::South,
        tile: Tile::new(Tile::Z3),
        is_tsumogiri: false,
    });
    let action = client.handle_event(&draw_event(Tile::M5));
    if let Some(t) = discarded_tile(&action) {
        assert_ne!(t.get(), Tile::Z3, "定石無効時は対子の現物を崩さない");
    }
}

#[test]
fn test_six_block_hand_dismantles_dead_kanchan_first() {
    // #149/#151/#153 の連動:
    // 6ブロック（M234 S789 Z5Z5 P1P2 S2S4 P78）の手で、
    // S3が3枚見えて死んだ嵌張(S2S4)を最優先で整理する。
    // 両面(P78)と唯一の雀頭(Z5Z5)は守る。
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
    // S3 が3枚場に出る → S2S4 は死にターツ
    for _ in 0..3 {
        client.handle_event(&ServerEvent::TileDiscarded {
            player: Wind::West,
            tile: Tile::new(Tile::S3),
            is_tsumogiri: true,
        });
    }
    let action = client.handle_event(&draw_event(Tile::P8));

    let tile = discarded_tile(&action).expect("expected a hand discard");
    assert!(
        tile.get() == Tile::S2 || tile.get() == Tile::S4,
        "死に嵌張(S2S4)を整理すべき, got {tile:?}"
    );
}

/// 么九牌 n 種 + 中張牌で14枚の配牌を作る
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
    // #160: 么九牌10種の手では么九牌を守り、中張牌から切る。
    // ルートロックがないと一般形向聴数に引かれて么九牌を切ってしまう。
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::East, orphan_rich_hand(10)));
    let action = client.handle_event(&draw_event(Tile::P6));

    let tile = discarded_tile(&action);
    // ツモ切り（P6）か手牌の中張牌切りなら正しい
    if let Some(t) = tile {
        assert!(
            !t.is_1_9_honour(),
            "国士無双ルートでは么九牌を切らない, got {t:?}"
        );
    }
}

#[test]
fn test_nine_terminals_continues_with_ten_kinds() {
    // #160: 么九牌10種以上は性格によらず国士無双を狙って続行
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let action = nine_terminals_action(config, orphan_rich_hand(10), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));
}

#[test]
fn test_nine_terminals_nine_kinds_depends_on_situation() {
    // 9種: 平場のバランス型は流局を選ぶ
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let action = nine_terminals_action(config, orphan_rich_hand(9), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: true })
    ));

    // 9種: 高打点型は国士狙いで続行
    let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue);
    let action = nine_terminals_action(config, orphan_rich_hand(9), [25000; 4]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));

    // 9種: 大きく負けていれば（#159）バランス型でも続行
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let action = nine_terminals_action(config, orphan_rich_hand(9), [8000, 42000, 25000, 25000]);
    assert!(matches!(
        action,
        Some(ClientAction::NineTerminals { declare: false })
    ));
}

#[test]
fn test_nine_terminals_without_heuristics_uses_personality() {
    // 定石無効時は従来どおり: HighValue のみ続行
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
    // 打牌・他プレイヤーツモ等はアクション不要なので None
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    let events = [
        ServerEvent::TileDiscarded {
            player: Wind::South,
            tile: Tile::new(Tile::M1),
            is_tsumogiri: false,
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
    // HighValue はチーしない → Pass を返す
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
    // 役牌ポンは向聴数が下がれば Normal レベルでも鳴く
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // Z5（白）×2 を持つ一向聴の手牌: M123+P456+S789完成+Z5Z5雀頭+Z2Z3孤立
    // → Z5 ポンで向聴数 1→0 に下がる
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
    // ポンで向聴数が下がらない場合は Pass
    // 国士無双テンパイ（13孤立牌+対子）では向聴数=0だが、
    // Z5 をポンすると closed=false になり nm 向聴数が大幅に上がる
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    // 12種の孤立牌 + Z5×2: 国士无双テンパイ(向聴数=0)
    // Z5 ポン後は closed 制約外れて向聴数が大幅に上昇する
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

/// 役なしになる鳴きの機会を作る共通手牌（M9ポンで向聴数は下がるが役がない）
///
/// 浮き牌は客風牌（南家にとって役牌でない Z3/Z4）にして、
/// 数牌の再分解による意図しない聴牌を防ぐ。
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
    // #162: 向聴数が下がっても役の見込みがない鳴きはしない
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, yakuless_pon_hand()));
    let action = client.handle_event(&pon_call_event(Tile::M9));

    assert!(matches!(action, Some(ClientAction::Pass)));
}

#[test]
fn test_yakuless_pon_called_without_heuristics() {
    // 定石無効時は従来どおり鳴く（A/B比較のベースライン維持）
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, yakuless_pon_hand()));
    let action = client.handle_event(&pon_call_event(Tile::M9));

    assert!(matches!(action, Some(ClientAction::Pon { .. })));
}

#[test]
fn test_weak_level_also_avoids_yakuless_pon() {
    // #162 は弱以上: Weakレベルでも役なし鳴きはしない
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);

    client.handle_event(&game_started_event(Wind::South, yakuless_pon_hand()));
    let action = client.handle_event(&pon_call_event(Tile::M9));

    assert!(matches!(action, Some(ClientAction::Pass)));
}

#[test]
fn test_high_value_pons_yakuhai() {
    // #163: 役牌対子のポンは性格（鳴き積極度）によらず行う
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

    // HighValue は鳴き積極度 0.2 で、従来は役牌すら鳴かなかった
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::HighValue);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand.clone()));
    let action = client.handle_event(&pon_call_event(Tile::Z5));
    assert!(matches!(action, Some(ClientAction::Pon { .. })));

    // 定石無効時は従来どおりパス
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
    // #164: 么九牌が3枚残る手から喰いタン目当てのチーをしない（中以上）
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

    // Normal: 厳しい条件で見込みなし → パス
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand.clone()));
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // Weak: 緩い条件（么九牌3枚以下）なら鳴く
    let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::South, hand));
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Chi { .. })));
}

#[test]
fn test_cheap_distant_chi_suppressed() {
    // #165: 子の、打点要素のない2向聴超の仕掛けは控える
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

    // ドラなし: 安くて遠い → パス
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand: hand.clone(),
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::Z5)], // ドラ(發)は手牌にない
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
    });
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // ドラ2枚あり: 打点見込みがあるので鳴く
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&ServerEvent::GameStarted {
        seat_wind: Wind::South,
        hand,
        scores: [25000; 4],
        round_wind: Wind::East,
        dora_indicators: vec![Tile::new(Tile::P6)], // ドラは P7（手牌に2枚...P7,P8のP7）
        round_number: 0,
        total_rounds: 4,
        honba: 0,
        riichi_sticks: 0,
    });
    let action = client.handle_event(&chi_call_event(Tile::M4, [Tile::M2, Tile::M3]));
    assert!(matches!(action, Some(ClientAction::Chi { .. })));
}

#[test]
fn test_toitoi_pon_requires_four_blocks() {
    // #157: 対々和狙いのポンは「副露+対子・刻子が4ブロック以上」のときだけ
    let s9_pon = Meld {
        tiles: vec![Tile::new(Tile::S9); 3],
        category: MeldType::Pon,
        from: MeldFrom::Unknown,
        called_tile: Some(Tile::new(Tile::S9)),
    };

    // 3ブロック相当（副露1 + M9M9 + P1P1）から M9 ポン → 役の見込みなし → パス
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

    // 5ブロック相当（副露1 + 対子4）から M9 ポン → 対々和の見込みあり → 鳴く
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
    // #166: 4副露目（裸単騎）になるポンはしない
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

    // 定石有効: パス
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    client.state.player_melds[0] = melds.clone();
    let action = client.handle_event(&pon_call_event(Tile::S3));
    assert!(matches!(action, Some(ClientAction::Pass)));

    // 定石無効: 従来どおり鳴く
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    client.state.player_melds[0] = melds;
    let action = client.handle_event(&pon_call_event(Tile::S3));
    assert!(matches!(action, Some(ClientAction::Pon { .. })));
}

#[test]
fn test_normal_level_avoids_hand_breaking_ankan() {
    // #167: 手を壊すカン（向聴数が悪化）は中レベル以上では行わない
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

    // 定石有効: S5×4 は S456+S555 に使われているのでカンせず打牌
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    let action = client.handle_event(&draw_event);
    assert!(
        matches!(action, Some(ClientAction::Discard { .. })),
        "expected discard instead of hand-breaking kan, got {action:?}"
    );

    // 定石無効: 従来の Normal はカンしてしまう
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced).without_heuristics();
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand));
    let action = client.handle_event(&draw_event);
    assert!(matches!(action, Some(ClientAction::Kan { .. })));
}

#[test]
fn test_ankan_suppressed_during_opponent_riichi() {
    // #167: 他家リーチ中、聴牌維持にならないカンはしない
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

    // リーチなし: 向聴数を保つカンなので実行する
    let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
    let mut client = CpuClient::new(config);
    client.handle_event(&game_started_event(Wind::East, hand.clone()));
    let action = client.handle_event(&draw_event);
    assert!(matches!(action, Some(ClientAction::Kan { .. })));

    // 他家リーチあり: カン後も聴牌しないのでカンしない
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
    // 大明カンの場合、Strong+HighValue 以外はパス
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
