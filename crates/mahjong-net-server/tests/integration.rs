//! Integration tests.
//!
//! Starts the server in-process and connects headless tokio-tungstenite
//! clients (tsumogiri bots) to exercise lobby operations and game flow.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mahjong_core::settings::Settings;
use mahjong_net_server::app;
use mahjong_net_server::room::RoomConfig;
use mahjong_server::protocol::net::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mahjong_server::protocol::{ClientAction, ServerEvent};
use mahjong_server::table::GameLength;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// Shortened timer configuration for tests.
fn fast_config() -> RoomConfig {
    RoomConfig {
        ready_timeout: Duration::from_millis(200),
        lobby_timeout: Duration::from_secs(30),
        abandoned_timeout: Duration::from_secs(5),
        // Kept long so the auto-advance never interferes with the tests.
        action_timeout: Some(Duration::from_secs(30)),
        // No delay and fine ticks: tests run nearly instantly.
        cpu_action_delay: Duration::ZERO,
        tick_interval: Duration::from_millis(1),
    }
}

/// Starts the server in-process and returns its listen address.
async fn start_server(config: RoomConfig) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app(config).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

/// Checks final-score consistency: deposits left on the table at game
/// end go to no one, so the total equals the starting sum minus
/// 1000 x the remaining deposits.
fn assert_scores_consistent(scores: [i32; 4]) {
    let sum: i32 = scores.iter().sum();
    assert!(
        sum <= 25000 * 4,
        "score total exceeds the initial points: {scores:?}"
    );
    assert_eq!(
        (25000 * 4 - sum) % 1000,
        0,
        "score-total difference is not a multiple of the deposit value: {scores:?}"
    );
}

/// Summarizes a message for the diagnostic trace.
fn summarize(msg: &ServerMessage) -> String {
    match msg {
        ServerMessage::Event(e) => match e {
            ServerEvent::GameStarted { round_number, .. } => {
                format!("GameStarted(round={round_number})")
            }
            ServerEvent::TileDrawn { can_tsumo, .. } => format!("TileDrawn(can_tsumo={can_tsumo})"),
            ServerEvent::TileDiscarded { player, .. } => format!("TileDiscarded({player:?})"),
            ServerEvent::CallAvailable { .. } => "CallAvailable".to_string(),
            ServerEvent::PlayerCalled {
                player, call_type, ..
            } => format!("PlayerCalled({player:?},{call_type:?})"),
            ServerEvent::NineTerminalsAvailable => "NineTerminalsAvailable".to_string(),
            ServerEvent::RoundWon { winner, .. } => format!("RoundWon({winner:?})"),
            ServerEvent::RoundDraw { reason, .. } => format!("RoundDraw({reason:?})"),
            other => format!("{other:?}").chars().take(30).collect(),
        },
        ServerMessage::GameOver { .. } => "GameOver".to_string(),
        ServerMessage::Error { code, message } => format!("Error({code:?},{message})"),
        other => format!("{other:?}").chars().take(30).collect(),
    }
}

/// Test WebSocket client.
struct TestClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl TestClient {
    async fn connect(addr: SocketAddr) -> Self {
        let (ws, _) = connect_async(format!("ws://{addr}/ws")).await.unwrap();
        TestClient { ws }
    }

    async fn send(&mut self, msg: &ClientMessage) {
        let json = msg.to_json().unwrap();
        self.ws.send(Message::text(json)).await.unwrap();
    }

    /// Receives the next ServerMessage, skipping Ping/Pong.
    ///
    /// The timeout is generous to avoid false failures on loaded CI.
    async fn recv(&mut self) -> ServerMessage {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(30), self.ws.next())
                .await
                .expect("receive timed out")
                .expect("connection closed")
                .expect("WebSocket error");
            match frame {
                Message::Text(text) => {
                    return ServerMessage::from_json(text.as_str()).expect("invalid JSON");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => panic!("connection closed"),
                other => panic!("unexpected frame: {other:?}"),
            }
        }
    }

    /// Reads until an Error message arrives and returns its code.
    async fn recv_error(&mut self) -> ErrorCode {
        for _ in 0..100 {
            if let ServerMessage::Error { code, .. } = self.recv().await {
                return code;
            }
        }
        panic!("Error message did not arrive");
    }

    /// Sends Hello and returns Welcome's session token.
    async fn hello(&mut self, name: &str) -> String {
        self.hello_with_token(name, None).await
    }

    /// Sends Hello with a session token and returns Welcome's token.
    async fn hello_with_token(&mut self, name: &str, token: Option<String>) -> String {
        self.send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            session_token: token,
            display_name: name.to_string(),
        })
        .await;
        match self.recv().await {
            ServerMessage::Welcome { session_token, .. } => session_token,
            other => panic!("expected Welcome message, got {other:?}"),
        }
    }

    /// Creates a room and returns its code.
    async fn create_room(&mut self) -> String {
        self.send(&ClientMessage::CreateRoom {
            length: GameLength::EastOnly,
            rules: Settings::new(),
        })
        .await;
        match self.recv().await {
            ServerMessage::RoomState { code, .. } => code,
            other => panic!("expected RoomState message, got {other:?}"),
        }
    }

    /// Creates a three-player room and returns its code.
    async fn create_sanma_room(&mut self) -> String {
        self.send(&ClientMessage::CreateRoom {
            length: GameLength::EastOnly,
            rules: Settings {
                three_player: true,
                ..Settings::new()
            },
        })
        .await;
        match self.recv().await {
            ServerMessage::RoomState { code, rules, .. } => {
                assert!(
                    rules.three_player,
                    "three_player is not set in a three-player room's RoomState"
                );
                code
            }
            other => panic!("expected RoomState message, got {other:?}"),
        }
    }

    /// Drains buffered messages, cut off by 50ms of quiet.
    ///
    /// Back-to-back events like `TileDrawn` + `NineTerminalsAvailable`
    /// arrive in separate frames, so decisions are made on the batch.
    async fn recv_batch(&mut self) -> Vec<ServerMessage> {
        let mut batch = vec![self.recv().await];
        loop {
            let frame = match tokio::time::timeout(Duration::from_millis(50), self.ws.next()).await
            {
                Ok(Some(Ok(frame))) => frame,
                // Quiet or disconnected; the next recv detects the latter.
                Err(_) | Ok(None) | Ok(Some(Err(_))) => break,
            };
            match frame {
                Message::Text(text) => {
                    batch.push(ServerMessage::from_json(text.as_str()).expect("invalid JSON"));
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                _ => break,
            }
        }
        batch
    }

    /// Plays as a tsumogiri bot until GameOver.
    ///
    /// With `send_ready` false, no ReadyNextRound is sent and the
    /// server's auto-advance takes over.
    async fn play_until_game_over(&mut self, send_ready: bool) -> [i32; 4] {
        loop {
            let batch = self.recv_batch().await;
            // Diagnostic trace, shown only on panic.
            for msg in &batch {
                println!("[bot] recv {}", summarize(msg));
            }

            // During a nine-terminals offer the phase is
            // WaitForNineTerminals, so discarding on the concurrent
            // TileDrawn would be invalid; declining makes the server
            // re-send TileDrawn to prompt the discard.
            let nine_terminals = batch
                .iter()
                .any(|m| matches!(m, ServerMessage::Event(ServerEvent::NineTerminalsAvailable)));
            if nine_terminals {
                self.send(&ClientMessage::Action(ClientAction::NineTerminals {
                    declare: false,
                }))
                .await;
            }

            for msg in batch {
                match msg {
                    ServerMessage::Event(event) => match event {
                        ServerEvent::TileDrawn { can_tsumo, .. } if !nine_terminals => {
                            let action = if can_tsumo {
                                ClientAction::Tsumo
                            } else {
                                ClientAction::Discard { tile: None }
                            };
                            self.send(&ClientMessage::Action(action)).await;
                        }
                        ServerEvent::CallAvailable { .. } => {
                            self.send(&ClientMessage::Action(ClientAction::Pass)).await;
                        }
                        ServerEvent::RoundWon { .. } | ServerEvent::RoundDraw { .. }
                            if send_ready =>
                        {
                            self.send(&ClientMessage::ReadyNextRound).await;
                        }
                        _ => {}
                    },
                    ServerMessage::GameOver { final_scores } => return final_scores,
                    ServerMessage::Error {
                        code: ErrorCode::InvalidAction,
                        ..
                    } => {
                        // The action lost a race (e.g. call resolution);
                        // harmless for a tsumogiri bot.
                    }
                    ServerMessage::Error { code, message } => {
                        panic!("unexpected error: {code:?} {message}");
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Two humans plus two CPUs must finish an East-only game.
#[tokio::test]
async fn test_full_game_with_two_humans() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        guest.hello("Guest").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;

        match guest.recv().await {
            ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
            other => panic!("expected RoomState message, got {other:?}"),
        }
        match host.recv().await {
            ServerMessage::RoomState { seats, .. } => {
                assert!(matches!(
                    seats[1],
                    mahjong_server::protocol::net::SeatInfo::Human { .. }
                ));
            }
            other => panic!("expected RoomState message, got {other:?}"),
        }

        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        let (host_scores, guest_scores) = tokio::join!(
            host.play_until_game_over(true),
            guest.play_until_game_over(true),
        );

        // Both observe the same, consistent final scores.
        assert_eq!(host_scores, guest_scores);
        assert_scores_consistent(host_scores);
    })
    .await
    .expect("test timed out");
}

/// The auto-advance must reach GameOver even when nobody sends
/// ReadyNextRound.
#[tokio::test]
async fn test_ready_timeout_auto_advances() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        host.create_room().await;
        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        let scores = host.play_until_game_over(false).await;
        assert_scores_consistent(scores);
    })
    .await
    .expect("test timed out");
}

/// The host's CPU configs must reach the seats.
#[tokio::test]
async fn test_host_chosen_cpu_configs_apply() {
    use mahjong_server::cpu::client::{CpuLevel, CpuPersonality};
    use mahjong_server::protocol::net::{CpuSpec, SeatInfo};

    tokio::time::timeout(Duration::from_secs(30), async {
        let addr = start_server(fast_config()).await;
        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        host.create_room().await;

        let specs = [
            CpuSpec {
                level: CpuLevel::Strong,
                personality: CpuPersonality::Defensive,
            },
            CpuSpec {
                level: CpuLevel::Weak,
                personality: CpuPersonality::Speedy,
            },
            CpuSpec {
                level: CpuLevel::Normal,
                personality: CpuPersonality::HighValue,
            },
        ];
        host.send(&ClientMessage::StartGame {
            cpu_configs: Some(specs),
        })
        .await;

        let seats = loop {
            if let ServerMessage::RoomState { seats, .. } = host.recv().await {
                break seats;
            }
        };

        assert!(matches!(seats[0], SeatInfo::Human { .. }));
        // Seats 1-3 carry the host's three configs; seats are shuffled,
        // so check the multiset rather than positions.
        for spec in &specs {
            let count = seats[1..]
                .iter()
                .filter(|s| {
                    matches!(s, SeatInfo::Cpu { level, personality }
                        if *level == spec.level && *personality == spec.personality)
                })
                .count();
            assert_eq!(
                count, 1,
                "exactly one CPU seat should have the requested configuration {spec:?}"
            );
        }
    })
    .await
    .expect("test timed out");
}

/// The host's SetCpuConfigs must reach everyone's RoomState (#245).
#[tokio::test]
async fn test_set_cpu_configs_shared_in_lobby() {
    use mahjong_server::cpu::client::{CpuLevel, CpuPersonality};
    use mahjong_server::protocol::net::{CpuSpec, SeatInfo};

    tokio::time::timeout(Duration::from_secs(30), async {
        let addr = start_server(fast_config()).await;
        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        guest.hello("Guest").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        // Skip the join RoomStates (they carry the default configs).
        match guest.recv().await {
            ServerMessage::RoomState { cpu_configs, .. } => {
                assert!(
                    cpu_configs.is_some(),
                    "lobby RoomState should include the CPU configuration"
                );
            }
            other => panic!("expected RoomState message, got {other:?}"),
        }
        host.recv().await;

        let specs = [
            CpuSpec {
                level: CpuLevel::Strong,
                personality: CpuPersonality::Defensive,
            },
            CpuSpec {
                level: CpuLevel::Weak,
                personality: CpuPersonality::Speedy,
            },
            CpuSpec {
                level: CpuLevel::Normal,
                personality: CpuPersonality::HighValue,
            },
        ];

        // Non-hosts must not be able to change the configs.
        guest
            .send(&ClientMessage::SetCpuConfigs { cpu_configs: specs })
            .await;
        assert_eq!(guest.recv_error().await, ErrorCode::NotHost);

        // The host's change reaches everyone as a fresh RoomState.
        host.send(&ClientMessage::SetCpuConfigs { cpu_configs: specs })
            .await;
        for client in [&mut host, &mut guest] {
            match client.recv().await {
                ServerMessage::RoomState {
                    cpu_configs, seats, ..
                } => {
                    assert_eq!(cpu_configs, Some(specs));
                    // Pre-game, empty seats stay Empty; the UI decorates
                    // them.
                    assert!(matches!(seats[2], SeatInfo::Empty));
                }
                other => panic!("expected RoomState message, got {other:?}"),
            }
        }
    })
    .await
    .expect("test timed out");
}

/// A protocol-version mismatch errors with VersionMismatch.
#[tokio::test]
async fn test_version_mismatch() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client
        .send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION + 1,
            session_token: None,
            display_name: "Old Client".to_string(),
        })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::VersionMismatch);
}

/// A first message other than Hello errors with BadMessage.
#[tokio::test]
async fn test_message_before_hello() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client
        .send(&ClientMessage::StartGame { cpu_configs: None })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::BadMessage);
}

/// Joining a nonexistent room errors with RoomNotFound.
#[tokio::test]
async fn test_join_unknown_room() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client.hello("Lost Client").await;
    client
        .send(&ClientMessage::JoinRoom {
            code: "ZZZZZZ".to_string(),
        })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::RoomNotFound);
}

/// Joining a full room errors with RoomFull.
#[tokio::test]
async fn test_room_full() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    let mut guests = Vec::new();
    for i in 0..3 {
        let mut guest = TestClient::connect(addr).await;
        guest.hello(&format!("Guest {i}")).await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        match guest.recv().await {
            ServerMessage::RoomState { .. } => {}
            other => panic!("expected RoomState message, got {other:?}"),
        }
        guests.push(guest);
    }

    let mut fifth = TestClient::connect(addr).await;
    fifth.hello("Fifth Player").await;
    fifth
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(fifth.recv_error().await, ErrorCode::RoomFull);
}

/// A three-player room fills at three; the fourth gets RoomFull.
#[tokio::test]
async fn test_sanma_room_full_at_three() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("Host").await;
    let code = host.create_sanma_room().await;

    let mut guests = Vec::new();
    for i in 0..2 {
        let mut guest = TestClient::connect(addr).await;
        guest.hello(&format!("Guest {i}")).await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        match guest.recv().await {
            ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, i + 1),
            other => panic!("expected RoomState message, got {other:?}"),
        }
        guests.push(guest);
    }

    let mut fourth = TestClient::connect(addr).await;
    fourth.hello("Fourth Player").await;
    fourth
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(fourth.recv_error().await, ErrorCode::RoomFull);
}

/// A three-player room with two humans and one CPU must finish an
/// East-only game (East 1-3).
#[tokio::test]
async fn test_sanma_full_game_with_two_humans() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        let code = host.create_sanma_room().await;

        let mut guest = TestClient::connect(addr).await;
        guest.hello("Guest").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        match guest.recv().await {
            ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
            other => panic!("expected RoomState message, got {other:?}"),
        }
        match host.recv().await {
            ServerMessage::RoomState { .. } => {}
            other => panic!("expected RoomState message, got {other:?}"),
        }

        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        let (host_scores, guest_scores) = tokio::join!(
            host.play_until_game_over(true),
            guest.play_until_game_over(true),
        );

        assert_eq!(host_scores, guest_scores);
        // The dummy seat's score stays 0.
        assert_eq!(
            host_scores[3], 0,
            "seat 3 has points in a three-player game"
        );
        // The total is at most 35000 x 3, short only by whole deposits.
        let sum: i32 = host_scores.iter().sum();
        assert!(
            sum <= 35000 * 3,
            "score total exceeds the initial points: {host_scores:?}"
        );
        assert_eq!(
            (35000 * 3 - sum) % 1000,
            0,
            "score-total difference is not a multiple of the deposit value: {host_scores:?}"
        );
    })
    .await
    .expect("test timed out");
}

/// StartGame from a non-host errors with NotHost.
#[tokio::test]
async fn test_non_host_cannot_start() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    let mut guest = TestClient::connect(addr).await;
    guest.hello("Guest").await;
    guest
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    guest
        .send(&ClientMessage::StartGame { cpu_configs: None })
        .await;
    assert_eq!(guest.recv_error().await, ErrorCode::NotHost);
}

/// An out-of-turn action errors with InvalidAction.
#[tokio::test]
async fn test_out_of_turn_action_rejected() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    let mut guest = TestClient::connect(addr).await;
    guest.hello("Guest").await;
    guest
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;

    host.recv().await;
    host.send(&ClientMessage::StartGame { cpu_configs: None })
        .await;

    // The dealer is random, and a CPU dealer races ahead with no
    // discard delay, so "the host acts first" cannot be assumed.
    // Whoever receives their own TileDrawn holds the turn; wait for
    // that, then check the other side's action is rejected.
    let host_turn;
    loop {
        tokio::select! {
            msg = host.recv() => {
                if matches!(msg, ServerMessage::Event(ServerEvent::TileDrawn { .. })) {
                    host_turn = true;
                    break;
                }
            }
            msg = guest.recv() => {
                if matches!(msg, ServerMessage::Event(ServerEvent::TileDrawn { .. })) {
                    host_turn = false;
                    break;
                }
            }
        }
    }

    let non_dealer = if host_turn { &mut guest } else { &mut host };
    non_dealer
        .send(&ClientMessage::Action(ClientAction::Discard { tile: None }))
        .await;
    assert_eq!(non_dealer.recv_error().await, ErrorCode::InvalidAction);
}

/// Joining after the game starts errors with GameInProgress.
#[tokio::test]
async fn test_join_after_start_rejected() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("Host").await;
    let code = host.create_room().await;
    host.send(&ClientMessage::StartGame { cpu_configs: None })
        .await;

    let mut late = TestClient::connect(addr).await;
    late.hello("Late Player").await;
    late.send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(late.recv_error().await, ErrorCode::GameInProgress);
}

/// A room that never starts is discarded on expiry.
#[tokio::test]
async fn test_lobby_room_expires() {
    let config = RoomConfig {
        lobby_timeout: Duration::from_millis(200),
        ..fast_config()
    };
    let addr = start_server(config).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    tokio::time::sleep(Duration::from_millis(600)).await;

    let mut guest = TestClient::connect(addr).await;
    guest.hello("Guest").await;
    guest
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(guest.recv_error().await, ErrorCode::RoomNotFound);
}

/// The game must finish with a CPU substituting for a mid-game
/// disconnect.
#[tokio::test]
async fn test_disconnect_mid_game_cpu_takes_over() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        guest.hello("Guest").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;

        host.recv().await;
        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        // The guest disconnects after seeing the game start.
        loop {
            if let ServerMessage::Event(ServerEvent::GameStarted { .. }) = guest.recv().await {
                break;
            }
        }
        drop(guest);

        // The host plays to the end; the CPU covers the guest's seat.
        let scores = host.play_until_game_over(true).await;
        assert_scores_consistent(scores);
    })
    .await
    .expect("test timed out");
}

/// The game must finish despite an AFK player, via the turn timeout.
#[tokio::test]
async fn test_action_timeout_auto_acts() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let config = RoomConfig {
            action_timeout: Some(Duration::from_millis(100)),
            ..fast_config()
        };
        let addr = start_server(config).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("AFK Host").await;
        host.create_room().await;
        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        // The host only receives, never acts; the server's default
        // actions (tsumogiri/pass) keep the game moving. The 100ms limit
        // shows as 0 seconds (enforcement uses real time).
        let mut saw_turn_timer = false;
        loop {
            match host.recv().await {
                ServerMessage::TurnTimer { .. } => {
                    saw_turn_timer = true;
                }
                ServerMessage::GameOver { final_scores } => {
                    assert_scores_consistent(final_scores);
                    break;
                }
                _ => {}
            }
        }
        assert!(saw_turn_timer, "TurnTimer never arrived");
    })
    .await
    .expect("test timed out");
}

/// Spamming invalid actions must not extend the turn timer.
///
/// The deadline used to be re-armed on every processed message, so
/// sending faster than the limit stalled the game forever. Regression:
/// the deadline persists while the same wait continues.
#[tokio::test]
async fn test_action_timeout_not_extended_by_invalid_actions() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let config = RoomConfig {
            action_timeout: Some(Duration::from_millis(500)),
            ..fast_config()
        };
        let addr = start_server(config).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        host.create_room().await;
        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        loop {
            if let ServerMessage::Event(ServerEvent::TileDrawn { .. }) = host.recv().await {
                break;
            }
        }

        // Spam invalid actions faster than the limit without
        // discarding; the timeout still fires and the server discards.
        let start = std::time::Instant::now();
        loop {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the deadline kept being extended and never timed out"
            );
            host.send(&ClientMessage::Action(ClientAction::Ron)).await;
            let deadline = tokio::time::sleep(Duration::from_millis(100));
            tokio::pin!(deadline);
            let mut auto_discarded = false;
            loop {
                tokio::select! {
                    msg = host.recv() => {
                        if let ServerMessage::Event(ServerEvent::TileDiscarded { .. }) = msg {
                            auto_discarded = true;
                            break;
                        }
                    }
                    _ = &mut deadline => break,
                }
            }
            if auto_discarded {
                break;
            }
        }
    })
    .await
    .expect("test timed out");
}

/// Too many join attempts from one IP get RateLimited.
#[tokio::test]
async fn test_join_rate_limit() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client.hello("Spammer").await;

    // Up to the cap (10), a missing room answers RoomNotFound.
    for _ in 0..10 {
        client
            .send(&ClientMessage::JoinRoom {
                code: "ZZZZZZ".to_string(),
            })
            .await;
        assert_eq!(client.recv_error().await, ErrorCode::RoomNotFound);
    }

    // The 11th attempt is rate limited.
    client
        .send(&ClientMessage::JoinRoom {
            code: "ZZZZZZ".to_string(),
        })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::RateLimited);
}

/// A disconnected player must rejoin by token and resync via Resync.
///
/// The host keeps playing while the guest disconnects and rejoins; the
/// game stalls unless the host keeps taking turns, so both run
/// concurrently under `tokio::join!`.
#[tokio::test]
async fn test_reconnect_resyncs_and_resumes() {
    tokio::time::timeout(Duration::from_secs(120), async {
        // Delay the CPUs slightly so the game outlives the rejoin;
        // immediate progress would end it during the 300ms sleep.
        let config = RoomConfig {
            cpu_action_delay: Duration::from_millis(50),
            tick_interval: Duration::from_millis(10),
            ..fast_config()
        };
        let addr = start_server(config).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        let guest_token = guest.hello("Guest").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;

        host.recv().await;
        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;

        // Host: keep playing to the end.
        let host_fut = host.play_until_game_over(true);

        // Guest: see the start, disconnect, rejoin, verify the Resync,
        // disconnect again.
        let guest_fut = async {
            loop {
                if let ServerMessage::Event(ServerEvent::GameStarted { .. }) = guest.recv().await {
                    break;
                }
            }
            drop(guest);
            // Let the CPU substitution advance a little.
            tokio::time::sleep(Duration::from_millis(300)).await;

            let mut rejoin = TestClient::connect(addr).await;
            rejoin.hello_with_token("Guest", Some(guest_token)).await;
            rejoin
                .send(&ClientMessage::JoinRoom { code: code.clone() })
                .await;

            // Receive RoomState plus the Resync (current-hand replay).
            let mut saw_room_state = false;
            let mut resync_events = None;
            for _ in 0..100 {
                match rejoin.recv().await {
                    ServerMessage::RoomState { your_seat, .. } => {
                        assert_eq!(
                            your_seat, 1,
                            "the rejoined client received a different seat"
                        );
                        saw_room_state = true;
                    }
                    ServerMessage::Resync { events } => {
                        resync_events = Some(events);
                        break;
                    }
                    _ => {}
                }
            }
            assert!(saw_room_state, "RoomState did not arrive after rejoining");
            let events = resync_events.expect("Resync did not arrive");
            // The replay starts at the current hand's GameStarted;
            // histories reset per hand.
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, ServerEvent::GameStarted { .. }))
                    .count(),
                1,
                "Resync should contain exactly one GameStarted event"
            );

            // Verification done, disconnect again without acting; the
            // CPU substitutes and the host finishes the game.
            drop(rejoin);
        };

        let (scores, ()) = tokio::join!(host_fut, guest_fut);
        assert_scores_consistent(scores);
    })
    .await
    .expect("test timed out");
}

/// A stale disconnect notice from an old connection must not disconnect
/// a reconnected seat; the ordered case (disconnect, then reconnect) is
/// verified to keep the game going.
#[tokio::test]
async fn test_reconnect_keeps_seat_connected() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("Host").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        let guest_token = guest.hello("Guest").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        host.recv().await;
        host.send(&ClientMessage::StartGame { cpu_configs: None })
            .await;
        loop {
            if let ServerMessage::Event(ServerEvent::GameStarted { .. }) = guest.recv().await {
                break;
            }
        }
        drop(guest);
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut rejoin = TestClient::connect(addr).await;
        rejoin.hello_with_token("Guest", Some(guest_token)).await;
        rejoin
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        // Skip ahead to the Resync.
        loop {
            if let ServerMessage::Resync { .. } = rejoin.recv().await {
                break;
            }
        }

        // The host is notified of the reconnection.
        let mut saw_reconnect = false;
        for _ in 0..50 {
            if let ServerMessage::PlayerConnectionChanged {
                seat: 1,
                connected: true,
            } = host.recv().await
            {
                saw_reconnect = true;
                break;
            }
        }
        assert!(
            saw_reconnect,
            "the host did not receive a reconnection notification"
        );
    })
    .await
    .expect("test timed out");
}
