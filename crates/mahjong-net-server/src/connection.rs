//! WebSocket connection handling: the Hello/Welcome handshake, lobby
//! operations (create/join), and post-join message relay. Each
//! connection runs two tasks: the main read task and a write task.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use mahjong_server::protocol::net::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mahjong_server::table::GameSettings;
use rand::RngExt;
use tokio::sync::{mpsc, oneshot};

use crate::AppState;
use crate::room::RoomMsg;

/// Idle time before the connection is dropped.
///
/// The server pings every 30 seconds and clients (browsers, tungstenite)
/// pong automatically, so a live connection is never silent this long.
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Ping interval.
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Per-connection send buffer, in messages.
const OUT_BUFFER: usize = 256;

/// Maximum WebSocket frame/message size in bytes.
const MAX_MESSAGE_SIZE: usize = 4 * 1024;

/// Inbound messages allowed per second.
const MAX_MSG_PER_SEC: usize = 20;

/// Strikes (bad messages, rate violations) tolerated before dropping.
const MAX_STRIKES: u32 = 10;

/// Generates a session token: 128 random bits as hex.
fn generate_token() -> String {
    format!("{:032x}", rand::rng().random::<u128>())
}

/// Whether a client-supplied token could have been issued by this server.
fn is_valid_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Header the Fly proxy adds to replayed requests; used to stop
/// forwarding loops.
const FLY_REPLAY_SRC: HeaderName = HeaderName::from_static("fly-replay-src");

/// Original client address supplied by Fly's HTTP proxy.
const FLY_CLIENT_IP: HeaderName = HeaderName::from_static("fly-client-ip");

/// Query parameters of `/ws`.
///
/// `room` is the code to join or rejoin, used in multi-machine setups to
/// locate the owning machine before the WebSocket upgrade. Joining still
/// happens via the post-upgrade `JoinRoom` message, so the parameter is
/// optional (its absence means room creation or a local-room join).
#[derive(serde::Deserialize)]
pub struct WsQuery {
    room: Option<String>,
}

/// The `/ws` handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    headers: HeaderMap,
) -> Response {
    // Origin restriction, only when ALLOWED_ORIGIN is set.
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !origin_allowed(state.allowed_origin.as_deref(), origin) {
        tracing::warn!(?origin, "rejected connection from disallowed origin");
        return StatusCode::FORBIDDEN.into_response();
    }

    let peer_ip = effective_client_ip(&headers, addr.ip(), state.peers.machine_id().is_some());

    // Forward to the owning machine if another one has the room.
    if let Some(code) = query.room.as_deref()
        && let Some(replay) = maybe_replay(&state, code, &headers).await
    {
        return replay;
    }

    // Cap frame/message sizes against oversized payloads.
    let ws = ws
        .max_message_size(MAX_MESSAGE_SIZE)
        .max_frame_size(MAX_MESSAGE_SIZE);
    ws.on_upgrade(move |socket| handle_socket(socket, peer_ip, state))
}

/// Locates the room code and returns a fly-replay response when another
/// machine owns it.
///
/// `None` upgrades normally (room is local, room not found, or already
/// replayed). A missing room is deliberately not an error here: the room
/// could close between lookup and join, so errors flow through the
/// `JoinRoom` reply as before.
async fn maybe_replay(state: &AppState, code: &str, headers: &HeaderMap) -> Option<Response> {
    let code = crate::lobby::normalize_code(code);
    // Skip peer queries for malformed codes: this path passes neither
    // auth nor rate limiting, so keep it cheap.
    if !crate::lobby::is_valid_code(&code) {
        return None;
    }
    if state.lobby.get(&code).is_some() {
        return None;
    }
    // Never re-forward a replayed request (loop prevention); if the room
    // closed right after the replay, the normal RoomNotFound flow
    // handles it.
    if headers.contains_key(FLY_REPLAY_SRC) {
        return None;
    }
    let machine_id = state.peers.find_room(&code).await?;
    tracing::info!(code, machine_id, "replaying connection to room owner");
    Some(
        (
            StatusCode::NO_CONTENT,
            [(
                HeaderName::from_static("fly-replay"),
                format!("instance={machine_id}"),
            )],
        )
            .into_response(),
    )
}

/// Whether the Origin is allowed: None allows all, Some requires an
/// exact Origin-header match.
fn origin_allowed(allowed: Option<&str>, origin: Option<&str>) -> bool {
    match allowed {
        None => true,
        Some(allowed) => origin == Some(allowed),
    }
}

/// Selects the rate-limit key, trusting proxy metadata only on Fly.
fn effective_client_ip(headers: &HeaderMap, direct_ip: IpAddr, trust_fly: bool) -> IpAddr {
    if !trust_fly {
        return direct_ip;
    }
    headers
        .get(FLY_CLIENT_IP)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(direct_ip)
}

async fn handle_socket(socket: WebSocket, peer_ip: IpAddr, state: AppState) {
    let (sender, receiver) = socket.split();
    let (out_tx, out_rx) = mpsc::channel::<ServerMessage>(OUT_BUFFER);

    let writer = tokio::spawn(write_loop(sender, out_rx));

    let mut conn = Connection {
        receiver,
        out_tx,
        state,
        peer_ip,
        recent_msgs: VecDeque::new(),
        strikes: 0,
    };
    conn.run().await;

    // Dropping out_tx makes the write task send Close and exit.
    drop(conn);
    let _ = writer.await;
}

/// The write task: sends queued messages as JSON and pings periodically.
async fn write_loop(
    mut sender: SplitSink<WebSocket, Message>,
    mut out_rx: mpsc::Receiver<ServerMessage>,
) {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // The first tick fires immediately; discard it.
    ping.tick().await;

    loop {
        tokio::select! {
            msg = out_rx.recv() => match msg {
                Some(msg) => {
                    let json = match msg.to_json() {
                        Ok(json) => json,
                        Err(e) => {
                            tracing::error!("failed to encode message: {e}");
                            continue;
                        }
                    };
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                None => {
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
            },
            _ = ping.tick() => {
                if sender.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Read outcome.
enum Read {
    Msg(ClientMessage),
    Closed,
}

/// How the post-join relay loop ended.
enum InRoomOutcome {
    /// The connection dropped (task exits)
    Closed,
    /// The client left (back to the lobby)
    LeftRoom,
}

struct Connection {
    receiver: SplitStream<WebSocket>,
    out_tx: mpsc::Sender<ServerMessage>,
    state: AppState,
    /// Peer IP, the rate-limit key
    peer_ip: IpAddr,
    /// Receive timestamps within the last second, for rate metering
    recent_msgs: VecDeque<Instant>,
    /// Strike count (bad messages, rate violations)
    strikes: u32,
}

impl Connection {
    async fn run(&mut self) {
        // --- Handshake ---
        let (token, name) = match self.read().await {
            Read::Msg(ClientMessage::Hello {
                protocol_version,
                session_token,
                display_name,
            }) => {
                if protocol_version != PROTOCOL_VERSION {
                    self.send_error(
                        ErrorCode::VersionMismatch,
                        &format!("server protocol version is {PROTOCOL_VERSION}"),
                    )
                    .await;
                    return;
                }
                let token = session_token
                    .filter(|token| is_valid_token(token))
                    .unwrap_or_else(generate_token);
                (token, display_name)
            }
            Read::Msg(_) => {
                self.send_error(ErrorCode::BadMessage, "expected Hello")
                    .await;
                return;
            }
            Read::Closed => return,
        };

        self.send(ServerMessage::Welcome {
            session_token: token.clone(),
            protocol_version: PROTOCOL_VERSION,
        })
        .await;

        // --- Lobby <-> room ---
        loop {
            let msg = match self.read().await {
                Read::Msg(msg) => msg,
                Read::Closed => return,
            };

            let room_tx = match msg {
                ClientMessage::CreateRoom { length, rules } => {
                    if !self.allow_room_entry() {
                        self.send_error(ErrorCode::RateLimited, "too many room attempts")
                            .await;
                        continue;
                    }
                    // The host's rule settings are adopted wholesale; the
                    // starting score is the server's call (25000/35000).
                    let settings = GameSettings::with_rules(length, rules);
                    let (_code, room_tx) = self
                        .state
                        .lobby
                        .create_room(settings, &self.state.peers)
                        .await;
                    Some(room_tx)
                }
                ClientMessage::JoinRoom { code } => {
                    if !self.allow_room_entry() {
                        self.send_error(ErrorCode::RateLimited, "too many room attempts")
                            .await;
                        continue;
                    }
                    let found = self.state.lobby.get(&code);
                    if found.is_none() {
                        self.send_error(ErrorCode::RoomNotFound, "no such room")
                            .await;
                    }
                    found
                }
                ClientMessage::Hello { .. } => {
                    self.send_error(ErrorCode::BadMessage, "already greeted")
                        .await;
                    None
                }
                _ => {
                    self.send_error(ErrorCode::NotInRoom, "join a room first")
                        .await;
                    None
                }
            };

            let Some(room_tx) = room_tx else {
                continue;
            };

            // --- Join ---
            let (reply_tx, reply_rx) = oneshot::channel();
            let join = RoomMsg::Join {
                name: name.clone(),
                token: token.clone(),
                tx: self.out_tx.clone(),
                reply: reply_tx,
            };
            if room_tx.send(join).await.is_err() {
                self.send_error(ErrorCode::RoomNotFound, "room closed")
                    .await;
                continue;
            }
            let (seat, conn_gen) = match reply_rx.await {
                Ok(Ok(assigned)) => assigned,
                Ok(Err(code)) => {
                    self.send_error(code, "join rejected").await;
                    continue;
                }
                Err(_) => {
                    self.send_error(ErrorCode::RoomNotFound, "room closed")
                        .await;
                    continue;
                }
            };

            // --- Relay loop ---
            match self.relay(room_tx, seat, conn_gen).await {
                InRoomOutcome::Closed => return,
                InRoomOutcome::LeftRoom => continue,
            }
        }
    }

    /// Post-join: relays client messages into the room.
    async fn relay(
        &mut self,
        room_tx: mpsc::Sender<RoomMsg>,
        seat: usize,
        conn_gen: u64,
    ) -> InRoomOutcome {
        loop {
            let msg = match self.read().await {
                Read::Msg(msg) => msg,
                Read::Closed => {
                    let _ = room_tx.send(RoomMsg::Disconnected { seat, conn_gen }).await;
                    return InRoomOutcome::Closed;
                }
            };

            match msg {
                ClientMessage::LeaveRoom => {
                    let _ = room_tx.send(RoomMsg::Leave { seat, conn_gen }).await;
                    return InRoomOutcome::LeftRoom;
                }
                ClientMessage::Hello { .. }
                | ClientMessage::CreateRoom { .. }
                | ClientMessage::JoinRoom { .. } => {
                    self.send_error(ErrorCode::BadMessage, "already in a room")
                        .await;
                }
                msg => {
                    if room_tx
                        .send(RoomMsg::FromSeat {
                            seat,
                            conn_gen,
                            msg,
                        })
                        .await
                        .is_err()
                    {
                        // The room closed.
                        self.send_error(ErrorCode::RoomNotFound, "room closed")
                            .await;
                        return InRoomOutcome::LeftRoom;
                    }
                }
            }
        }
    }

    /// Reads the next client message.
    ///
    /// Malformed frames get a `BadMessage` reply and reading continues;
    /// `IDLE_TIMEOUT` of silence counts as a disconnect; `MAX_STRIKES`
    /// accumulated violations drop the connection.
    async fn read(&mut self) -> Read {
        loop {
            let frame = match tokio::time::timeout(IDLE_TIMEOUT, self.receiver.next()).await {
                Ok(Some(Ok(frame))) => frame,
                // Stream end, protocol errors, and timeouts all count
                // as disconnects.
                Ok(Some(Err(_))) | Ok(None) | Err(_) => return Read::Closed,
            };

            match frame {
                Message::Text(text) => {
                    // Rate violations count as strikes.
                    if self.over_message_rate() {
                        self.send_error(ErrorCode::RateLimited, "too many messages")
                            .await;
                        if self.strike() {
                            return Read::Closed;
                        }
                        continue;
                    }
                    match ClientMessage::from_json(text.as_str()) {
                        Ok(msg) => return Read::Msg(msg),
                        Err(_) => {
                            self.send_error(ErrorCode::BadMessage, "invalid message")
                                .await;
                            if self.strike() {
                                return Read::Closed;
                            }
                        }
                    }
                }
                Message::Binary(_) => {
                    self.send_error(ErrorCode::BadMessage, "binary frames not supported")
                        .await;
                    if self.strike() {
                        return Read::Closed;
                    }
                }
                // The layer below answers pings automatically.
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(_) => return Read::Closed,
            }
        }
    }

    /// Whether inbound messages exceed the per-second cap; each call
    /// records now and drops entries older than one second.
    fn over_message_rate(&mut self) -> bool {
        let now = Instant::now();
        let cutoff = now.checked_sub(Duration::from_secs(1));
        while let Some(&front) = self.recent_msgs.front() {
            match cutoff {
                Some(cutoff) if front < cutoff => {
                    self.recent_msgs.pop_front();
                }
                _ => break,
            }
        }
        self.recent_msgs.push_back(now);
        self.recent_msgs.len() > MAX_MSG_PER_SEC
    }

    /// Adds a strike; true (drop the connection) once the cap is hit.
    fn strike(&mut self) -> bool {
        self.strikes += 1;
        self.strikes >= MAX_STRIKES
    }

    /// Checks the per-IP join rate limit; false when exceeded.
    fn allow_room_entry(&self) -> bool {
        self.state.rate_limiter.check(self.peer_ip)
    }

    async fn send(&self, msg: ServerMessage) {
        let _ = self.out_tx.send(msg).await;
    }

    async fn send_error(&self, code: ErrorCode, message: &str) {
        self.send(ServerMessage::Error {
            code,
            message: message.to_string(),
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::{FLY_CLIENT_IP, effective_client_ip, is_valid_token, origin_allowed};
    use axum::http::{HeaderMap, HeaderValue};
    use std::net::IpAddr;

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn test_session_token_format() {
        assert!(is_valid_token("0123456789abcdef0123456789ABCDEF"));
        assert!(!is_valid_token(""));
        assert!(!is_valid_token("deadbeef"));
        assert!(!is_valid_token("0123456789abcdef0123456789abcdeg"));
    }

    #[test]
    fn test_origin_allowed_when_unset_allows_all() {
        assert!(origin_allowed(None, Some("https://example.com")));
        assert!(origin_allowed(None, None));
    }

    #[test]
    fn test_origin_allowed_requires_exact_match() {
        let allowed = Some("https://mahjong.example.com");
        assert!(origin_allowed(allowed, Some("https://mahjong.example.com")));
        // Mismatches and missing headers are rejected.
        assert!(!origin_allowed(allowed, Some("https://evil.example.com")));
        assert!(!origin_allowed(allowed, None));
    }

    #[test]
    fn test_effective_client_ip_trusts_fly_header_only_on_fly() {
        let mut headers = HeaderMap::new();
        headers.insert(FLY_CLIENT_IP, HeaderValue::from_static("203.0.113.9"));
        let direct = ip("10.0.0.1");

        assert_eq!(
            effective_client_ip(&headers, direct, true),
            ip("203.0.113.9")
        );
        assert_eq!(effective_client_ip(&headers, direct, false), direct);
    }

    #[test]
    fn test_effective_client_ip_rejects_invalid_fly_header() {
        let mut headers = HeaderMap::new();
        headers.insert(FLY_CLIENT_IP, HeaderValue::from_static("not-an-ip"));
        let direct = ip("10.0.0.1");

        assert_eq!(effective_client_ip(&headers, direct, true), direct);
        assert_eq!(effective_client_ip(&HeaderMap::new(), direct, true), direct);
    }
}
