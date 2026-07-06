//! 複数マシン構成の統合テスト
//!
//! 2台の「マシン」（プロセス内サーバ）を相互にピア登録して起動し、
//! ルーム所在の照会と fly-replay 転送の判断を検証する。
//! 実際の転送は Fly Proxy が行うため、ここでは「正しい fly-replay
//! ヘッダを返すこと」と「転送後のリクエストが正常に処理されること」を
//! それぞれ確認する。

use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use mahjong_core::settings::Settings;
use mahjong_net_server::lobby::Lobby;
use mahjong_net_server::peers::Peers;
use mahjong_net_server::ratelimit::RateLimiter;
use mahjong_net_server::room::RoomConfig;
use mahjong_net_server::{AppState, app_with_state, internal_app};
use mahjong_server::protocol::net::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mahjong_server::table::GameLength;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite};

/// テスト用マシン（プロセス内サーバ）
struct Machine {
    /// 公開リスナー（/ws）のアドレス
    public: SocketAddr,
    /// 内部リスナー（/internal/rooms/{code}）のアドレス
    internal: SocketAddr,
}

/// 状態からマシンを起動する（内部リスナーは bind 済みのものを使う）
async fn serve_machine(state: AppState, internal: tokio::net::TcpListener) -> Machine {
    let internal_addr = internal.local_addr().unwrap();
    let internal_state = state.clone();
    tokio::spawn(async move {
        axum::serve(internal, internal_app(internal_state))
            .await
            .unwrap();
    });

    let public = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let public_addr = public.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            public,
            app_with_state(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    Machine {
        public: public_addr,
        internal: internal_addr,
    }
}

/// 2台のマシンを相互にピア登録して起動する
async fn start_two_machines() -> (Machine, Machine) {
    // 先に内部リスナーを bind してアドレスを確定させ、相互参照させる
    let internal_a = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let internal_b = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr_a = internal_a.local_addr().unwrap().to_string();
    let addr_b = internal_b.local_addr().unwrap().to_string();

    let state = |machine_id: &str, peer: String| AppState {
        lobby: Lobby::new(RoomConfig::default()),
        rate_limiter: RateLimiter::new(),
        allowed_origin: None,
        peers: Peers::with_static(Some(machine_id), vec![peer]),
    };

    let machine_a = serve_machine(state("machinea", addr_b), internal_a).await;
    let machine_b = serve_machine(state("machineb", addr_a), internal_b).await;
    (machine_a, machine_b)
}

/// テスト用 WebSocket クライアント（ロビー操作のみ）
struct TestClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl TestClient {
    async fn connect(url: &str) -> Self {
        Self::connect_with(url.into_client_request().unwrap()).await
    }

    async fn connect_with(request: tungstenite::handshake::client::Request) -> Self {
        let (ws, _) = connect_async(request).await.expect("接続に失敗した");
        TestClient { ws }
    }

    async fn send(&mut self, msg: &ClientMessage) {
        let json = msg.to_json().unwrap();
        self.ws.send(Message::text(json)).await.unwrap();
    }

    async fn recv(&mut self) -> ServerMessage {
        loop {
            let frame = tokio::time::timeout(std::time::Duration::from_secs(30), self.ws.next())
                .await
                .expect("受信がタイムアウトした")
                .expect("接続が閉じられた")
                .expect("WebSocketエラー");
            match frame {
                Message::Text(text) => {
                    return ServerMessage::from_json(text.as_str()).expect("不正なJSON");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("予期しないフレーム: {other:?}"),
            }
        }
    }

    async fn hello(&mut self, name: &str) {
        self.send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            session_token: None,
            display_name: name.to_string(),
        })
        .await;
        match self.recv().await {
            ServerMessage::Welcome { .. } => {}
            other => panic!("Welcomeでないメッセージ: {other:?}"),
        }
    }

    async fn create_room(&mut self) -> String {
        self.send(&ClientMessage::CreateRoom {
            length: GameLength::EastOnly,
            rules: Settings::new(),
        })
        .await;
        match self.recv().await {
            ServerMessage::RoomState { code, .. } => code,
            other => panic!("RoomStateでないメッセージ: {other:?}"),
        }
    }

    async fn join_room(&mut self, code: &str) -> ServerMessage {
        self.send(&ClientMessage::JoinRoom {
            code: code.to_string(),
        })
        .await;
        self.recv().await
    }
}

/// 他マシンのルームコード付きで接続すると fly-replay 応答が返る
#[tokio::test]
async fn test_join_foreign_room_returns_fly_replay() {
    let (machine_a, machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    // マシンBへ ?room=CODE 付きで接続すると、アップグレードせずに
    // マシンAへの fly-replay 応答が返る
    let request = format!("ws://{}/ws?room={}", machine_b.public, code)
        .into_client_request()
        .unwrap();
    match connect_async(request).await {
        Err(tungstenite::Error::Http(response)) => {
            let replay = response
                .headers()
                .get("fly-replay")
                .expect("fly-replay ヘッダが無い")
                .to_str()
                .unwrap();
            assert_eq!(replay, "instance=machinea");
        }
        Ok(_) => panic!("アップグレードされてしまった（fly-replay が返るべき）"),
        Err(other) => panic!("予期しないエラー: {other:?}"),
    }
}

/// 自マシンのルームコード付きの接続は通常どおりアップグレードして入室できる
#[tokio::test]
async fn test_join_local_room_with_query_param() {
    let (machine_a, _machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    let mut guest =
        TestClient::connect(&format!("ws://{}/ws?room={}", machine_a.public, code)).await;
    guest.hello("ゲスト").await;
    match guest.join_room(&code).await {
        ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
        other => panic!("RoomStateでないメッセージ: {other:?}"),
    }
}

/// 転送されてきたリクエスト（fly-replay-src 付き）は再転送されない
///
/// Fly Proxy による転送後の受信側の挙動を、ヘッダを付けた直接接続で模擬する。
/// ルームを所持するマシンでは入室でき、所持しないマシンでは（転送し直さず）
/// RoomNotFound になることを確認する。
#[tokio::test]
async fn test_replayed_request_is_not_replayed_again() {
    let (machine_a, machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    let with_replay_src = |addr: SocketAddr| {
        let mut request = format!("ws://{addr}/ws?room={code}")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "fly-replay-src",
            "instance=machineb;t=1751772000000000".parse().unwrap(),
        );
        request
    };

    // 所持マシンA: 転送後のリクエストとして正常に入室できる
    let mut guest = TestClient::connect_with(with_replay_src(machine_a.public)).await;
    guest.hello("ゲスト").await;
    match guest.join_room(&code).await {
        ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
        other => panic!("RoomStateでないメッセージ: {other:?}"),
    }

    // 非所持マシンB: 再転送せずアップグレードし、JoinRoom は RoomNotFound
    let mut lost = TestClient::connect_with(with_replay_src(machine_b.public)).await;
    lost.hello("迷子").await;
    match lost.join_room(&code).await {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::RoomNotFound),
        other => panic!("Errorでないメッセージ: {other:?}"),
    }
}

/// 存在しないコードや不正な形式のコードでは転送されず通常どおり接続できる
#[tokio::test]
async fn test_unknown_or_invalid_code_upgrades_normally() {
    let (machine_a, _machine_b) = start_two_machines().await;

    for room in ["ZZZZZZ", "abc", "%2F%2F", ""] {
        let mut client =
            TestClient::connect(&format!("ws://{}/ws?room={}", machine_a.public, room)).await;
        client.hello("探検者").await;
    }
}

/// ピア照会（find_room）がルームの所在を正しく返す
#[tokio::test]
async fn test_peer_lookup_finds_room_owner() {
    let (machine_a, _machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("ホスト").await;
    let code = host.create_room().await;

    // 第三者視点のピア構成からマシンAの内部リスナーへ照会する
    let observer = Peers::with_static(Some("observer1"), vec![machine_a.internal.to_string()]);
    assert_eq!(
        observer.find_room(&code).await,
        Some("machinea".to_string())
    );
    assert_eq!(observer.find_room("ZZZZZZ").await, None);
    // 小文字でも正規化されて見つかる（Lobby::get が正規化する）
    assert_eq!(
        observer.find_room(&code.to_ascii_lowercase()).await,
        Some("machinea".to_string())
    );
}

/// ピアが常に「所持している」と答えてもルーム作成が完了する（無限ループしない）
#[tokio::test]
async fn test_create_room_completes_with_lying_peer() {
    // どのコードにも 200 を返す偽ピア
    let liar = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let liar_addr = liar.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        let router = axum::Router::new().route(
            "/internal/rooms/{code}",
            axum::routing::get(|| async { "machinex" }),
        );
        axum::serve(liar, router).await.unwrap();
    });

    let state = AppState {
        lobby: Lobby::new(RoomConfig::default()),
        rate_limiter: RateLimiter::new(),
        allowed_origin: None,
        peers: Peers::with_static(Some("machinea"), vec![liar_addr]),
    };
    let internal = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let machine = serve_machine(state, internal).await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine.public)).await;
    host.hello("ホスト").await;
    // 衝突チェックの上限に達した後、ローカルの一意性のみで確定して完了する
    let code = host.create_room().await;
    assert_eq!(code.len(), 6);
}
