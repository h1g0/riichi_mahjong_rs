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
        // 既存テストの自動進行を阻害しないよう長めにする
        action_timeout: Some(Duration::from_secs(30)),
        // テストは遅延なし・細かいティックでほぼ即時に進める
        cpu_action_delay: Duration::ZERO,
        tick_interval: Duration::from_millis(1),
    }
}

/// サーバをプロセス内で起動し、リッスンアドレスを返す
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

/// 診断トレース用にメッセージを短い文字列へ要約する
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
        self.hello_with_token(name, None).await
    }

    /// セッショントークンを指定して Hello を送り、Welcome のトークンを返す
    async fn hello_with_token(&mut self, name: &str, token: Option<String>) -> String {
        self.send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            session_token: token,
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

    /// 受信済みのメッセージをまとめて取り出す（50msの静止で区切る）
    ///
    /// `TileDrawn` と `NineTerminalsAvailable` のように連続で送られる
    /// イベントは別フレームで届くため、まとめてから行動を判断する。
    async fn recv_batch(&mut self) -> Vec<ServerMessage> {
        let mut batch = vec![self.recv().await];
        loop {
            let frame = match tokio::time::timeout(Duration::from_millis(50), self.ws.next()).await
            {
                Ok(Some(Ok(frame))) => frame,
                // 静止 or 切断（切断は次の recv で検出する）
                Err(_) | Ok(None) | Ok(Some(Err(_))) => break,
            };
            match frame {
                Message::Text(text) => {
                    batch.push(ServerMessage::from_json(text.as_str()).expect("不正なJSON"));
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                _ => break,
            }
        }
        batch
    }

    /// ツモ切りボットとして GameOver まで打ち続ける
    ///
    /// `send_ready` が false の場合は局結果の確認（ReadyNextRound）を送らず、
    /// サーバ側の自動進行に任せる。
    async fn play_until_game_over(&mut self, send_ready: bool) -> [i32; 4] {
        loop {
            let batch = self.recv_batch().await;
            // 失敗時の診断用トレース（パニック時のみ表示される）
            for msg in &batch {
                println!("[bot] recv {}", summarize(msg));
            }

            // 九種九牌の選択があるターンはフェーズが WaitForNineTerminals の
            // ため、同時に届いた TileDrawn への打牌は無効になる。宣言を
            // 拒否すると、サーバが TileDrawn を再送して打牌を促す
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
                        // 鳴き解決などのレースで無効になったアクション。
                        // ツモ切りボットでは無害なので無視する。
                    }
                    ServerMessage::Error { code, message } => {
                        panic!("予期しないエラー: {code:?} {message}");
                    }
                    _ => {}
                }
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

/// 操作しない（AFK）プレイヤーがいてもタイムアウトで対局が完走することを確認する
#[tokio::test]
async fn test_action_timeout_auto_acts() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let config = RoomConfig {
            action_timeout: Some(Duration::from_millis(100)),
            ..fast_config()
        };
        let addr = start_server(config).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("AFKホスト").await;
        host.create_room().await;
        host.send(&ClientMessage::StartGame).await;

        // ホストは一切操作せず受信し続ける。
        // サーバが既定アクション（ツモ切り/パス）を代行して対局が進む。
        // 制限時間は 100ms なので表示秒数は 0 に丸められる（サーバは実時間で強制）
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
        assert!(saw_turn_timer, "TurnTimer が一度も届かなかった");
    })
    .await
    .expect("テスト全体がタイムアウトした");
}

/// 同一IPからの入室試行が多すぎると RateLimited で拒否されることを確認する
#[tokio::test]
async fn test_join_rate_limit() {
    let addr = start_server(fast_config()).await;
    let mut client = TestClient::connect(addr).await;
    client.hello("スパマー").await;

    // 上限（10回）までは存在しないルームとして RoomNotFound
    for _ in 0..10 {
        client
            .send(&ClientMessage::JoinRoom {
                code: "ZZZZZZ".to_string(),
            })
            .await;
        assert_eq!(client.recv_error().await, ErrorCode::RoomNotFound);
    }

    // 11回目はレート制限で拒否される
    client
        .send(&ClientMessage::JoinRoom {
            code: "ZZZZZZ".to_string(),
        })
        .await;
    assert_eq!(client.recv_error().await, ErrorCode::RateLimited);
}

/// 切断したプレイヤーがトークンで再入室し、Resync で状態を再同期できることを確認する
///
/// ホストは継続して打ち、その裏でゲストが切断→再入室する。ホストが手番を
/// 進め続けないと対局が止まるため、両者を `tokio::join!` で並行に動かす。
#[tokio::test]
async fn test_reconnect_resyncs_and_resumes() {
    tokio::time::timeout(Duration::from_secs(120), async {
        // CPU をわずかに遅延させ、再入室するまで対局が終わらないようにする
        // （即時進行だと 300ms のスリープ中に終局してしまう）
        let config = RoomConfig {
            cpu_action_delay: Duration::from_millis(50),
            tick_interval: Duration::from_millis(10),
            ..fast_config()
        };
        let addr = start_server(config).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("ホスト").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        let guest_token = guest.hello("ゲスト").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;

        host.recv().await; // ゲスト入室の RoomState
        host.send(&ClientMessage::StartGame).await;

        // ホスト: 最後まで打ち続ける
        let host_fut = host.play_until_game_over(true);

        // ゲスト: 開始確認 → 切断 → 再入室 → Resync 検証 → 再び切断
        let guest_fut = async {
            loop {
                if let ServerMessage::Event(ServerEvent::GameStarted { .. }) = guest.recv().await {
                    break;
                }
            }
            drop(guest);
            // CPU 代打ちで少し進める
            tokio::time::sleep(Duration::from_millis(300)).await;

            // トークンを提示して再入室する
            let mut rejoin = TestClient::connect(addr).await;
            rejoin.hello_with_token("ゲスト", Some(guest_token)).await;
            rejoin
                .send(&ClientMessage::JoinRoom { code: code.clone() })
                .await;

            // RoomState と Resync（現在の局の再生）を受け取る
            let mut saw_room_state = false;
            let mut resync_events = None;
            for _ in 0..100 {
                match rejoin.recv().await {
                    ServerMessage::RoomState { your_seat, .. } => {
                        assert_eq!(your_seat, 1, "再入室の座席が元と違う");
                        saw_room_state = true;
                    }
                    ServerMessage::Resync { events } => {
                        resync_events = Some(events);
                        break;
                    }
                    _ => {}
                }
            }
            assert!(saw_room_state, "再入室で RoomState が届かなかった");
            let events = resync_events.expect("Resync が届かなかった");
            // 再生は現在の局の GameStarted から始まる（履歴は局ごとにリセット）
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, ServerEvent::GameStarted { .. }))
                    .count(),
                1,
                "Resync に GameStarted がちょうど1つ含まれるべき"
            );

            // 検証が目的のため、再接続後の操作は行わず再び切断する
            // （CPU が代打ちして対局はホスト主導で完走する）
            drop(rejoin);
        };

        let (scores, ()) = tokio::join!(host_fut, guest_fut);
        assert_scores_consistent(scores);
    })
    .await
    .expect("テスト全体がタイムアウトした");
}

/// 古い接続からの遅延切断通知が再接続済みの座席を誤って切断しないことを確認する
///
/// ここでは順序どおり（切断 → 再接続）の正常系として、再接続後に対局が
/// 継続することを確認する。
#[tokio::test]
async fn test_reconnect_keeps_seat_connected() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let addr = start_server(fast_config()).await;

        let mut host = TestClient::connect(addr).await;
        host.hello("ホスト").await;
        let code = host.create_room().await;

        let mut guest = TestClient::connect(addr).await;
        let guest_token = guest.hello("ゲスト").await;
        guest
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        host.recv().await;
        host.send(&ClientMessage::StartGame).await;
        loop {
            if let ServerMessage::Event(ServerEvent::GameStarted { .. }) = guest.recv().await {
                break;
            }
        }
        drop(guest);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 再入室
        let mut rejoin = TestClient::connect(addr).await;
        rejoin.hello_with_token("ゲスト", Some(guest_token)).await;
        rejoin
            .send(&ClientMessage::JoinRoom { code: code.clone() })
            .await;
        // Resync まで読み飛ばす
        loop {
            if let ServerMessage::Resync { .. } = rejoin.recv().await {
                break;
            }
        }

        // ホストへ再接続が通知される
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
        assert!(saw_reconnect, "ホストに再接続通知が届かなかった");
    })
    .await
    .expect("テスト全体がタイムアウトした");
}
