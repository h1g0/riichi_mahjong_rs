//! 統合テスト
//!
//! サーバをプロセス内で起動し、tokio-tungstenite のヘッドレスクライアント
//! （ツモ切りボット）を接続して、ロビー操作と対局進行を検証する。

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mahjong_net_server::app;
use mahjong_net_server::room::RoomConfig;
use mahjong_server::protocol::net::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mahjong_server::protocol::{ClientAction, ServerEvent};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// テスト用の短いタイマー設定
fn fast_config() -> RoomConfig {
    RoomConfig {
        ready_timeout: Duration::from_millis(200),
        lobby_timeout: Duration::from_secs(30),
        abandoned_timeout: Duration::from_secs(5),
    }
}

/// サーバをプロセス内で起動し、リッスンアドレスを返す
async fn start_server(config: RoomConfig) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app(config)).await.unwrap();
    });
    addr
}

/// 最終得点の整合性を確認する
///
/// ゲーム終了時に場に残った供託リーチ棒は誰にも配られないため、
/// 総和は「初期点数の合計 − 1000 × 残り供託」になる。
fn assert_scores_consistent(scores: [i32; 4]) {
    let sum: i32 = scores.iter().sum();
    assert!(sum <= 25000 * 4, "総和が初期点数を超えている: {scores:?}");
    assert_eq!(
        (25000 * 4 - sum) % 1000,
        0,
        "総和の差が供託単位でない: {scores:?}"
    );
}

/// テスト用 WebSocket クライアント
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

    /// 次の ServerMessage を受信する（Ping/Pong は読み飛ばす）
    ///
    /// タイムアウトはCI等の高負荷環境での誤検知を避けるため長めに取る。
    async fn recv(&mut self) -> ServerMessage {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(30), self.ws.next())
                .await
                .expect("受信がタイムアウトした")
                .expect("接続が閉じられた")
                .expect("WebSocketエラー");
            match frame {
                Message::Text(text) => {
                    return ServerMessage::from_json(text.as_str()).expect("不正なJSON");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => panic!("接続が閉じられた"),
                other => panic!("予期しないフレーム: {other:?}"),
            }
        }
    }

    /// Error メッセージが届くまで読み、そのエラーコードを返す
    async fn recv_error(&mut self) -> ErrorCode {
        for _ in 0..100 {
            if let ServerMessage::Error { code, .. } = self.recv().await {
                return code;
            }
        }
        panic!("Errorメッセージが届かなかった");
    }

    /// Hello を送り、Welcome のセッショントークンを返す
    async fn hello(&mut self, name: &str) -> String {
        self.send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            session_token: None,
            display_name: name.to_string(),
        })
        .await;
        match self.recv().await {
            ServerMessage::Welcome { session_token, .. } => session_token,
            other => panic!("Welcomeでないメッセージ: {other:?}"),
        }
    }

    /// ルームを作成し、ルームコードを返す
    async fn create_room(&mut self) -> String {
        self.send(&ClientMessage::CreateRoom { round_count: 1 })
            .await;
        match self.recv().await {
            ServerMessage::RoomState { code, .. } => code,
            other => panic!("RoomStateでないメッセージ: {other:?}"),
        }
    }

    /// ツモ切りボットとして GameOver まで打ち続ける
    ///
    /// `send_ready` が false の場合は局結果の確認（ReadyNextRound）を送らず、
    /// サーバ側の自動進行に任せる。
    async fn play_until_game_over(&mut self, send_ready: bool) -> [i32; 4] {
        loop {
            match self.recv().await {
                ServerMessage::Event(event) => match event {
                    ServerEvent::TileDrawn { can_tsumo, .. } => {
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
                    ServerEvent::NineTerminalsAvailable => {
                        self.send(&ClientMessage::Action(ClientAction::NineTerminals {
                            declare: false,
                        }))
                        .await;
                    }
                    ServerEvent::RoundWon { .. } | ServerEvent::RoundDraw { .. } if send_ready => {
                        self.send(&ClientMessage::ReadyNextRound).await;
                    }
                    _ => {}
                },
                ServerMessage::GameOver { final_scores } => return final_scores,
                ServerMessage::Error { code, message } => {
                    panic!("予期しないエラー: {code:?} {message}");
                }
                _ => {}
            }
        }
    }
}

/// 2人の人間 + CPU2人で東風戦を最後まで打ち切れることを確認する
#[tokio::test]
async fn test_full_game_with_two_humans() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("ホスト").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        guest.hello("ゲスト").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;

        // ゲストは自分の入室を反映した RoomState を受け取る
        match guest.recv().await {
            ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
            other => panic!("RoomStateでないメッセージ: {other:?}"),
        }
        // ホストにも更新された RoomState が届く
        match host.recv().await {
            ServerMessage::RoomState { seats, .. } => {
                assert!(matches!(
                    seats[1],
                    mahjong_server::protocol::net::SeatInfo::Human { .. }
                ));
            }
            other => panic!("RoomStateでないメッセージ: {other:?}"),
        }

        host.send(&ClientMessage::StartGame).await;

        let (host_scores, guest_scores) = tokio::join!(
            host.play_until_game_over(true),
            guest.play_until_game_over(true),
        );

        // 両者が同じ最終得点を観測し、点数の整合性が取れている
        assert_eq!(host_scores, guest_scores);
        assert_scores_consistent(host_scores);
    })
    .await
    .expect("テスト全体がタイムアウトした");
}

/// ReadyNextRound を誰も送らなくても自動進行で GameOver まで到達することを確認する
#[tokio::test]
async fn test_ready_timeout_auto_advances() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("ホスト").await;
        host.create_room().await;
        host.send(&ClientMessage::StartGame).await;

        let scores = host.play_until_game_over(false).await;
        assert_scores_consistent(scores);
    })
    .await
    .expect("テスト全体がタイムアウトした");
}

/// プロトコルバージョン不一致は VersionMismatch エラーになる
#[tokio::test]
async fn test_version_mismatch() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client
        .send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION + 1,
            session_token: None,
            display_name: "古いクライアント".to_string(),
        })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::VersionMismatch);
}

/// Hello 以外の最初のメッセージは BadMessage になる
#[tokio::test]
async fn test_message_before_hello() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client.send(&ClientMessage::StartGame).await;
    assert_eq!(client.recv_error().await, ErrorCode::BadMessage);
}

/// 存在しないルームコードへの参加は RoomNotFound になる
#[tokio::test]
async fn test_join_unknown_room() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client.hello("迷子").await;
    client
        .send(&ClientMessage::JoinRoom {
            code: "ZZZZZZ".to_string(),
        })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::RoomNotFound);
}

/// 満席のルームへの参加は RoomFull になる
#[tokio::test]
async fn test_room_full() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    let mut guests = Vec::new();
    for i in 0..3 {
        let mut guest = TestClient::connect(addr).await;
        guest.hello(&format!("ゲスト{i}")).await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        match guest.recv().await {
            ServerMessage::RoomState { .. } => {}
            other => panic!("RoomStateでないメッセージ: {other:?}"),
        }
        guests.push(guest);
    }

    let mut fifth = TestClient::connect(addr).await;
    fifth.hello("5人目").await;
    fifth
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(fifth.recv_error().await, ErrorCode::RoomFull);
}

/// ホスト以外の StartGame は NotHost になる
#[tokio::test]
async fn test_non_host_cannot_start() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    let mut guest = TestClient::connect(addr).await;
    guest.hello("ゲスト").await;
    guest
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    guest.send(&ClientMessage::StartGame).await;
    assert_eq!(guest.recv_error().await, ErrorCode::NotHost);
}

/// 手番でないプレイヤーのアクションは InvalidAction になる
#[tokio::test]
async fn test_out_of_turn_action_rejected() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    let mut guest = TestClient::connect(addr).await;
    guest.hello("ゲスト").await;
    guest
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;

    host.recv().await; // ゲスト入室の RoomState
    host.send(&ClientMessage::StartGame).await;

    // 開始直後の手番はホスト（座席0=親）。ゲストの打牌は拒否される
    guest
        .send(&ClientMessage::Action(ClientAction::Discard { tile: None }))
        .await;
    assert_eq!(guest.recv_error().await, ErrorCode::InvalidAction);
}

/// 対局開始後の参加は GameInProgress になる
#[tokio::test]
async fn test_join_after_start_rejected() {
    let addr = start_server(fast_config()).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;
    host.send(&ClientMessage::StartGame).await;

    let mut late = TestClient::connect(addr).await;
    late.hello("遅刻").await;
    late.send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(late.recv_error().await, ErrorCode::GameInProgress);
}

/// 対局が始まらないルームは期限切れで破棄される
#[tokio::test]
async fn test_lobby_room_expires() {
    let config = RoomConfig {
        lobby_timeout: Duration::from_millis(200),
        ..fast_config()
    };
    let addr = start_server(config).await;

    let mut host = TestClient::connect(addr).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    tokio::time::sleep(Duration::from_millis(600)).await;

    let mut guest = TestClient::connect(addr).await;
    guest.hello("ゲスト").await;
    guest
        .send(&ClientMessage::JoinRoom { code: code.clone() })
        .await;
    assert_eq!(guest.recv_error().await, ErrorCode::RoomNotFound);
}

/// 対局中に片方が切断しても CPU が代打ちして対局が完走することを確認する
#[tokio::test]
async fn test_disconnect_mid_game_cpu_takes_over() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("ホスト").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        guest.hello("ゲスト").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;

        host.recv().await; // ゲスト入室の RoomState
        host.send(&ClientMessage::StartGame).await;

        // ゲストはゲーム開始を確認してから切断する
        loop {
            if let ServerMessage::Event(ServerEvent::GameStarted { .. }) = guest.recv().await {
                break;
            }
        }
        drop(guest);

        // ホストは最後まで打ち切れる（ゲスト座席はCPUが代打ち）
        let scores = host.play_until_game_over(true).await;
        assert_scores_consistent(scores);
    })
    .await
    .expect("テスト全体がタイムアウトした");
}
