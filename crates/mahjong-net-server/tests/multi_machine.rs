//! Multi-machine integration tests.
//!
//! Starts two in-process "machines" registered as each other's peers and
//! verifies room lookup and the fly-replay decision. The Fly proxy does
//! the actual forwarding, so these tests check that the right fly-replay
//! header is returned and that a replayed request is handled correctly.

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

/// A test machine (in-process server).
struct Machine {
    /// Public listener (/ws) address
    public: SocketAddr,
    /// Internal listener (/internal/rooms/{code}) address
    internal: SocketAddr,
}

/// Starts a machine from state, using the pre-bound internal listener.
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

/// Starts two machines registered as each other's peers.
async fn start_two_machines() -> (Machine, Machine) {
    // Bind the internal listeners first so the addresses exist for
    // cross-registration.
    let internal_a = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let internal_b = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr_a = internal_a.local_addr().unwrap().to_string();
    let addr_b = internal_b.local_addr().unwrap().to_string();

    let state = |machine_id: &str, peer: String| AppState {
        lobby: Lobby::new(RoomConfig::default()),
        rate_limiter: RateLimiter::new(),
        allowed_origins: None,
        peers: Peers::with_static(Some(machine_id), vec![peer]),
    };

    let machine_a = serve_machine(state("machinea", addr_b), internal_a).await;
    let machine_b = serve_machine(state("machineb", addr_a), internal_b).await;
    (machine_a, machine_b)
}

/// Test WebSocket client (lobby operations only).
struct TestClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl TestClient {
    async fn connect(url: &str) -> Self {
        Self::connect_with(url.into_client_request().unwrap()).await
    }

    async fn connect_with(request: tungstenite::handshake::client::Request) -> Self {
        let (ws, _) = connect_async(request).await.expect("connection failed");
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
                .expect("receive timed out")
                .expect("connection closed")
                .expect("WebSocket error");
            match frame {
                Message::Text(text) => {
                    return ServerMessage::from_json(text.as_str()).expect("invalid JSON");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("unexpected frame: {other:?}"),
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
            other => panic!("expected Welcome message, got {other:?}"),
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
            other => panic!("expected RoomState message, got {other:?}"),
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

/// Connecting with another machine's room code yields a fly-replay
/// response.
#[tokio::test]
async fn test_join_foreign_room_returns_fly_replay() {
    let (machine_a, machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    // A ?room=CODE connection to machine B returns a fly-replay to
    // machine A instead of upgrading.
    let request = format!("ws://{}/ws?room={}", machine_b.public, code)
        .into_client_request()
        .unwrap();
    match connect_async(request).await {
        Err(tungstenite::Error::Http(response)) => {
            let replay = response
                .headers()
                .get("fly-replay")
                .expect("fly-replay header is missing")
                .to_str()
                .unwrap();
            assert_eq!(replay, "instance=machinea");
        }
        Ok(_) => panic!("connection was upgraded instead of returning fly-replay"),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

/// A connection with our own machine's room code upgrades and joins
/// normally.
#[tokio::test]
async fn test_join_local_room_with_query_param() {
    let (machine_a, _machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    let mut guest =
        TestClient::connect(&format!("ws://{}/ws?room={}", machine_a.public, code)).await;
    guest.hello("Guest").await;
    match guest.join_room(&code).await {
        ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
        other => panic!("expected RoomState message, got {other:?}"),
    }
}

/// A replayed request (with fly-replay-src) is never re-forwarded.
///
/// Simulates the post-replay receiving side with a direct connection
/// carrying the header: the owning machine joins fine, and the
/// non-owning machine yields RoomNotFound rather than forwarding again.
#[tokio::test]
async fn test_replayed_request_is_not_replayed_again() {
    let (machine_a, machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("Host").await;
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

    // Owning machine A: the replayed request joins normally.
    let mut guest = TestClient::connect_with(with_replay_src(machine_a.public)).await;
    guest.hello("Guest").await;
    match guest.join_room(&code).await {
        ServerMessage::RoomState { your_seat, .. } => assert_eq!(your_seat, 1),
        other => panic!("expected RoomState message, got {other:?}"),
    }

    // Non-owning machine B upgrades without re-forwarding;
    // JoinRoom yields RoomNotFound.
    let mut lost = TestClient::connect_with(with_replay_src(machine_b.public)).await;
    lost.hello("Lost Client").await;
    match lost.join_room(&code).await {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::RoomNotFound),
        other => panic!("expected Error message, got {other:?}"),
    }
}

/// Nonexistent or malformed codes connect normally with no forwarding.
#[tokio::test]
async fn test_unknown_or_invalid_code_upgrades_normally() {
    let (machine_a, _machine_b) = start_two_machines().await;

    for room in ["ZZZZZZ", "abc", "%2F%2F", ""] {
        let mut client =
            TestClient::connect(&format!("ws://{}/ws?room={}", machine_a.public, room)).await;
        client.hello("Explorer").await;
    }
}

/// find_room locates rooms correctly.
#[tokio::test]
async fn test_peer_lookup_finds_room_owner() {
    let (machine_a, _machine_b) = start_two_machines().await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine_a.public)).await;
    host.hello("Host").await;
    let code = host.create_room().await;

    // Query machine A's internal listener from a third party's
    // peer config.
    let observer = Peers::with_static(Some("observer1"), vec![machine_a.internal.to_string()]);
    assert_eq!(
        observer.find_room(&code).await,
        Some("machinea".to_string())
    );
    assert_eq!(observer.find_room("ZZZZZZ").await, None);
    // Lowercase codes normalize and still resolve (Lobby::get
    // normalizes).
    assert_eq!(
        observer.find_room(&code.to_ascii_lowercase()).await,
        Some("machinea".to_string())
    );
}

/// Room creation completes even against a peer that always claims
/// ownership (no infinite loop).
#[tokio::test]
async fn test_create_room_completes_with_lying_peer() {
    // A fake peer answering 200 for every code.
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
        allowed_origins: None,
        peers: Peers::with_static(Some("machinea"), vec![liar_addr]),
    };
    let internal = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let machine = serve_machine(state, internal).await;

    let mut host = TestClient::connect(&format!("ws://{}/ws", machine.public)).await;
    host.hello("Host").await;
    // Past the collision-check cap, local uniqueness decides and
    // creation completes.
    let code = host.create_room().await;
    assert_eq!(code.len(), 6);
}
