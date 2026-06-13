//! WebSocket 接続の処理
//!
//! ハンドシェイク（Hello/Welcome）、ロビー操作（ルーム作成・参加）、
//! 入室後のメッセージ中継を行う。1接続につき読み取りタスク（本体）と
//! 書き込みタスクの2つが動く。

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use mahjong_server::protocol::net::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mahjong_server::table::GameSettings;
use rand::RngExt;
use tokio::sync::{mpsc, oneshot};

use crate::AppState;
use crate::room::RoomMsg;

/// 受信が途絶えてから接続を切るまでの時間
///
/// サーバは30秒ごとに Ping を送り、クライアント（ブラウザ/tungstenite）は
/// 自動で Pong を返すため、生きている接続でこの時間無音になることはない。
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Ping の送信間隔
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// 接続ごとの送信バッファ（メッセージ数）
const OUT_BUFFER: usize = 256;

/// WebSocket フレーム/メッセージの最大サイズ（バイト）
const MAX_MESSAGE_SIZE: usize = 4 * 1024;

/// 1秒あたりに許す受信メッセージ数
const MAX_MSG_PER_SEC: usize = 20;

/// 接続を切るまでに許す違反（不正メッセージ・レート超過）の累計
const MAX_STRIKES: u32 = 10;

/// セッショントークンを生成する（128ビットのランダム16進文字列）
fn generate_token() -> String {
    format!("{:032x}", rand::rng().random::<u128>())
}

/// `/ws` ハンドラ
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    // Origin 制限（ALLOWED_ORIGIN 設定時のみ）
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !origin_allowed(state.allowed_origin.as_deref(), origin) {
        tracing::warn!(?origin, "rejected connection from disallowed origin");
        return StatusCode::FORBIDDEN.into_response();
    }

    // フレーム/メッセージサイズを制限して巨大ペイロードを防ぐ
    let ws = ws
        .max_message_size(MAX_MESSAGE_SIZE)
        .max_frame_size(MAX_MESSAGE_SIZE);
    ws.on_upgrade(move |socket| handle_socket(socket, addr.ip(), state))
}

/// 接続元 Origin が許可されるか判定する
///
/// `allowed` が None なら全許可。Some なら Origin ヘッダが一致した場合のみ許可。
fn origin_allowed(allowed: Option<&str>, origin: Option<&str>) -> bool {
    match allowed {
        None => true,
        Some(allowed) => origin == Some(allowed),
    }
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

    // out_tx を破棄すると書き込みタスクが Close を送って終了する
    drop(conn);
    let _ = writer.await;
}

/// 送信専用タスク: キューのメッセージを JSON で送り、定期的に Ping を打つ
async fn write_loop(
    mut sender: SplitSink<WebSocket, Message>,
    mut out_rx: mpsc::Receiver<ServerMessage>,
) {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // 最初の tick は即時発火するので読み捨てる
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

/// 読み取り結果
enum Read {
    Msg(ClientMessage),
    Closed,
}

/// 入室後の中継ループの終わり方
enum InRoomOutcome {
    /// 接続が切れた（タスク終了）
    Closed,
    /// 退出した（ロビーに戻る）
    LeftRoom,
}

struct Connection {
    receiver: SplitStream<WebSocket>,
    out_tx: mpsc::Sender<ServerMessage>,
    state: AppState,
    /// 接続元のIP（レート制限のキー）
    peer_ip: IpAddr,
    /// 直近1秒間の受信時刻（メッセージレート計測）
    recent_msgs: VecDeque<Instant>,
    /// 違反（不正メッセージ・レート超過）の累計
    strikes: u32,
}

impl Connection {
    async fn run(&mut self) {
        // --- ハンドシェイク ---
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
                (session_token.unwrap_or_else(generate_token), display_name)
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

        // --- ロビー ⇄ 入室 ---
        loop {
            let msg = match self.read().await {
                Read::Msg(msg) => msg,
                Read::Closed => return,
            };

            let room_tx = match msg {
                ClientMessage::CreateRoom { round_count } => {
                    if !self.allow_room_entry() {
                        self.send_error(ErrorCode::RateLimited, "too many room attempts")
                            .await;
                        continue;
                    }
                    if !(1..=2).contains(&round_count) {
                        self.send_error(ErrorCode::BadMessage, "round_count must be 1 or 2")
                            .await;
                        continue;
                    }
                    let settings = GameSettings {
                        round_count,
                        ..GameSettings::default()
                    };
                    let (_code, room_tx) = self.state.lobby.create_room(settings);
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

            // --- 入室 ---
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

            // --- 中継ループ ---
            match self.relay(room_tx, seat, conn_gen).await {
                InRoomOutcome::Closed => return,
                InRoomOutcome::LeftRoom => continue,
            }
        }
    }

    /// 入室後: クライアントのメッセージをルームへ中継する
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
                    let _ = room_tx.send(RoomMsg::Leave { seat }).await;
                    return InRoomOutcome::LeftRoom;
                }
                ClientMessage::Hello { .. }
                | ClientMessage::CreateRoom { .. }
                | ClientMessage::JoinRoom { .. } => {
                    self.send_error(ErrorCode::BadMessage, "already in a room")
                        .await;
                }
                msg => {
                    if room_tx.send(RoomMsg::FromSeat { seat, msg }).await.is_err() {
                        // ルームが閉じられた
                        self.send_error(ErrorCode::RoomNotFound, "room closed")
                            .await;
                        return InRoomOutcome::LeftRoom;
                    }
                }
            }
        }
    }

    /// 次のクライアントメッセージを読む
    ///
    /// 不正な形式のフレームには `BadMessage` を返して読み続ける。
    /// `IDLE_TIMEOUT` の間なにも届かなければ切断と判断する。
    /// 不正メッセージ・レート超過が累計 `MAX_STRIKES` に達したら切断する。
    async fn read(&mut self) -> Read {
        loop {
            let frame = match tokio::time::timeout(IDLE_TIMEOUT, self.receiver.next()).await {
                Ok(Some(Ok(frame))) => frame,
                // ストリーム終了・プロトコルエラー・タイムアウトはすべて切断扱い
                Ok(Some(Err(_))) | Ok(None) | Err(_) => return Read::Closed,
            };

            match frame {
                Message::Text(text) => {
                    // メッセージレートを超過したら違反として扱う
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
                // Ping への Pong 応答は下層が自動で行う
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(_) => return Read::Closed,
            }
        }
    }

    /// 受信メッセージが直近1秒の上限を超えているか判定する
    ///
    /// 呼び出しごとに現在時刻を記録し、1秒より古い記録を捨てる。
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

    /// 違反を1つ加算し、累計が上限に達したら true（切断すべき）を返す
    fn strike(&mut self) -> bool {
        self.strikes += 1;
        self.strikes >= MAX_STRIKES
    }

    /// IPごとの入室レート制限を確認する（超過なら false）
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
    use super::origin_allowed;

    #[test]
    fn test_origin_allowed_when_unset_allows_all() {
        assert!(origin_allowed(None, Some("https://example.com")));
        assert!(origin_allowed(None, None));
    }

    #[test]
    fn test_origin_allowed_requires_exact_match() {
        let allowed = Some("https://mahjong.example.com");
        assert!(origin_allowed(allowed, Some("https://mahjong.example.com")));
        // 不一致・欠落は拒否
        assert!(!origin_allowed(allowed, Some("https://evil.example.com")));
        assert!(!origin_allowed(allowed, None));
    }
}
