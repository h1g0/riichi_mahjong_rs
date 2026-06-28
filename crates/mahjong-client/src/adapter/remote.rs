//! リモートアダプター
//!
//! WebSocket 経由で mahjong-net-server と通信する。
//! ロビー操作（ルーム作成・参加・開始）と対局中のイベント中継を担う。
//!
//! 接続〜入室の流れ:
//! 1. `create_room` / `join_room` でトランスポートを開き、意図を保持する
//! 2. `Opened` を受けたら `Hello` を送る
//! 3. `Welcome` を受けたら保持していた `CreateRoom` / `JoinRoom` を送る
//! 4. `RoomState` で入室完了（`room()` が Some になる）
//! 5. ホストが `start_game` → `Event(GameStarted)` で対局開始

use mahjong_core::settings::Lang;
use mahjong_server::protocol::net::{
    ClientMessage, CpuSpec, ErrorCode, PROTOCOL_VERSION, SeatInfo, ServerMessage,
};
use mahjong_server::protocol::{ClientAction, ServerEvent};

use super::GameAdapter;
use crate::transport::{Transport, WsEvent};

/// 接続状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnStatus {
    /// 接続・ハンドシェイク中
    Connecting,
    /// 接続済み
    Connected,
    /// 切断された
    Disconnected,
}

/// ルームの表示用スナップショット
#[derive(Debug, Clone)]
pub struct RoomView {
    /// ルームコード
    pub code: String,
    /// 各座席の状態
    pub seats: [SeatInfo; 4],
    /// ホストの座席
    pub host_seat: usize,
    /// 自分の座席
    pub your_seat: usize,
}

impl RoomView {
    /// 自分がホストか
    pub fn is_host(&self) -> bool {
        self.your_seat == self.host_seat
    }
}

/// 直近のエラー
#[derive(Debug, Clone)]
pub struct RemoteError {
    /// サーバが返したエラーコード（通信層のエラーは None）
    pub code: Option<ErrorCode>,
    /// 説明（UI 表示用）
    pub message: String,
}

/// Welcome 受信後に送るロビー操作
enum LobbyIntent {
    Create { round_count: u8 },
    Join { code: String },
}

/// 自動再接続のバックオフ間隔（秒）。試行回数で頭打ちにする。
const RECONNECT_BACKOFF: [f64; 5] = [1.0, 2.0, 4.0, 8.0, 10.0];

/// 新しいトランスポートを生成する関数（再接続で使う）
type Connector = Box<dyn FnMut() -> Box<dyn Transport>>;

/// 現在時刻（秒）を返す関数
type Clock = Box<dyn Fn() -> f64>;

/// リモートアダプター: ネットワーク越しにサーバとやり取りする
pub struct RemoteAdapter {
    transport: Box<dyn Transport>,
    /// 再接続用に新しいトランスポートを作る
    connector: Connector,
    /// 現在時刻（秒）。再接続のバックオフ計測に使う
    clock: Clock,
    status: ConnStatus,
    display_name: String,
    session_token: Option<String>,
    pending_intent: Option<LobbyIntent>,
    room: Option<RoomView>,
    /// 再接続に使うルームコード（RoomState で判明する）
    room_code: Option<String>,
    events: Vec<ServerEvent>,
    last_error: Option<RemoteError>,
    game_started: bool,
    game_over: bool,
    ready_sent: bool,
    /// 自動再接続中か
    reconnecting: bool,
    /// 次の再接続を試みる時刻
    reconnect_at: Option<f64>,
    /// 再接続の試行回数
    reconnect_attempts: u32,
    /// 各座席の人間プレイヤーの接続状態（None = 人間以外/不明）
    peer_connected: [Option<bool>; 4],
    /// 手番の制限時間の期限（秒, clock 基準）。None なら表示しない
    turn_deadline: Option<f64>,
}

impl RemoteAdapter {
    /// トランスポート・コネクタ・時計を指定して作成する
    fn build(
        transport: Box<dyn Transport>,
        connector: Connector,
        clock: Clock,
        display_name: &str,
        intent: LobbyIntent,
    ) -> Self {
        RemoteAdapter {
            transport,
            connector,
            clock,
            status: ConnStatus::Connecting,
            display_name: display_name.to_string(),
            session_token: None,
            pending_intent: Some(intent),
            room: None,
            room_code: None,
            events: Vec::new(),
            last_error: None,
            game_started: false,
            game_over: false,
            ready_sent: false,
            reconnecting: false,
            reconnect_at: None,
            reconnect_attempts: 0,
            peer_connected: [None; 4],
            turn_deadline: None,
        }
    }

    /// サーバに接続してルームを作成する
    pub fn create_room(url: &str, display_name: &str, round_count: u8) -> Self {
        let (transport, connector) = Self::connector_for(url);
        Self::build(
            transport,
            connector,
            default_clock(),
            display_name,
            LobbyIntent::Create { round_count },
        )
    }

    /// サーバに接続して既存のルームに参加する
    pub fn join_room(url: &str, display_name: &str, code: &str) -> Self {
        let (transport, connector) = Self::connector_for(url);
        Self::build(
            transport,
            connector,
            default_clock(),
            display_name,
            LobbyIntent::Join {
                code: code.trim().to_ascii_uppercase(),
            },
        )
    }

    /// 指定URL用のコネクタと最初のトランスポートを作る
    fn connector_for(url: &str) -> (Box<dyn Transport>, Connector) {
        let url = url.to_string();
        let mut connector: Connector = Box::new(move || crate::transport::connect(&url));
        let transport = connector();
        (transport, connector)
    }

    /// 対局を開始する（ホストのみ有効。結果はサーバが判断する）
    ///
    /// `cpu_configs` でCPUの強さ・性格を指定する（`None` ならサーバ既定）。
    pub fn start_game(&mut self, cpu_configs: Option<[CpuSpec; 3]>) {
        self.send(&ClientMessage::StartGame { cpu_configs });
    }

    /// ルームから退出する
    pub fn leave_room(&mut self) {
        self.send(&ClientMessage::LeaveRoom);
        self.room = None;
    }

    /// 現在の接続状態
    pub fn status(&self) -> ConnStatus {
        self.status
    }

    /// 入室中のルーム情報
    pub fn room(&self) -> Option<&RoomView> {
        self.room.as_ref()
    }

    /// 対局が開始したか（GameStarted を受信したか）
    pub fn game_started(&self) -> bool {
        self.game_started
    }

    /// 直近のエラーを取り出す（取り出すとクリアされる）
    pub fn take_error(&mut self) -> Option<RemoteError> {
        self.last_error.take()
    }

    /// 受信を処理して内部状態を更新する
    fn pump(&mut self) {
        self.maybe_reconnect();
        for ws_event in self.transport.poll() {
            match ws_event {
                WsEvent::Opened => {
                    let hello = ClientMessage::Hello {
                        protocol_version: PROTOCOL_VERSION,
                        session_token: self.session_token.clone(),
                        display_name: self.display_name.clone(),
                    };
                    self.send(&hello);
                }
                WsEvent::Message(json) => match ServerMessage::from_json(&json) {
                    Ok(msg) => self.handle_server_message(msg),
                    Err(_) => {
                        self.last_error = Some(RemoteError {
                            code: None,
                            message: "サーバからの応答を解釈できません".to_string(),
                        });
                    }
                },
                WsEvent::Closed => self.handle_disconnect(None),
                WsEvent::Error(message) => self.handle_disconnect(Some(message)),
            }
        }
    }

    /// 切断・通信エラーを処理する
    ///
    /// 対局中は自動再接続を試みる（エラーは表に出さず「再接続中」を表示）。
    /// ロビーや対局終了後は通常の切断として扱う。
    fn handle_disconnect(&mut self, message: Option<String>) {
        if self.should_auto_reconnect() {
            // 一時的な切断: 再接続モードへ（既に再接続中なら継続）
            if !self.reconnecting {
                self.enter_reconnect();
            } else {
                self.status = ConnStatus::Disconnected;
            }
            return;
        }
        self.status = ConnStatus::Disconnected;
        if let Some(message) = message {
            self.last_error = Some(RemoteError {
                code: None,
                message,
            });
        }
    }

    /// 自動再接続すべき状況か（対局中で終了していない）
    fn should_auto_reconnect(&self) -> bool {
        self.game_started && !self.game_over && self.room_code.is_some()
    }

    /// 自動再接続モードに入る
    fn enter_reconnect(&mut self) {
        self.reconnecting = true;
        self.reconnect_attempts = 0;
        self.status = ConnStatus::Disconnected;
        self.reconnect_at = Some((self.clock)() + RECONNECT_BACKOFF[0]);
    }

    /// 再接続の時刻になっていれば新しい接続を張る
    fn maybe_reconnect(&mut self) {
        if !self.reconnecting {
            return;
        }
        let Some(at) = self.reconnect_at else {
            return;
        };
        if (self.clock)() < at {
            return;
        }
        let Some(code) = self.room_code.clone() else {
            // ルームコード不明なら再接続できない
            self.reconnecting = false;
            self.reconnect_at = None;
            return;
        };

        // 新しいトランスポートを張り、再入室をやり直す
        self.transport = (self.connector)();
        self.status = ConnStatus::Connecting;
        self.pending_intent = Some(LobbyIntent::Join { code });

        let idx = (self.reconnect_attempts as usize + 1).min(RECONNECT_BACKOFF.len() - 1);
        self.reconnect_attempts += 1;
        self.reconnect_at = Some((self.clock)() + RECONNECT_BACKOFF[idx]);
    }

    /// 再接続を断念すべき種類のエラーか
    fn is_terminal_reconnect_error(code: ErrorCode) -> bool {
        matches!(
            code,
            ErrorCode::RoomNotFound
                | ErrorCode::GameInProgress
                | ErrorCode::NotInRoom
                | ErrorCode::VersionMismatch
        )
    }

    fn handle_server_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::Welcome { session_token, .. } => {
                self.session_token = Some(session_token);
                self.status = ConnStatus::Connected;
                if let Some(intent) = self.pending_intent.take() {
                    let msg = match intent {
                        LobbyIntent::Create { round_count } => {
                            ClientMessage::CreateRoom { round_count }
                        }
                        LobbyIntent::Join { code } => ClientMessage::JoinRoom { code },
                    };
                    self.send(&msg);
                }
            }
            ServerMessage::RoomState {
                code,
                seats,
                host_seat,
                your_seat,
            } => {
                self.room_code = Some(code.clone());
                // 座席情報から人間プレイヤーの接続状態を取り込む
                for (i, info) in seats.iter().enumerate() {
                    self.peer_connected[i] = match info {
                        SeatInfo::Human { connected, .. } if i != your_seat => Some(*connected),
                        _ => None,
                    };
                }
                self.room = Some(RoomView {
                    code,
                    seats,
                    host_seat,
                    your_seat,
                });
            }
            ServerMessage::Event(event) => {
                if matches!(event, ServerEvent::GameStarted { .. }) {
                    self.game_started = true;
                    // 新しい局が始まったので次局確認を再送可能にする
                    self.ready_sent = false;
                }
                self.events.push(event);
            }
            ServerMessage::Resync { events } => {
                // 現在の局を最初から再生する。再接続が完了したので通常状態へ戻す。
                self.reconnecting = false;
                self.reconnect_at = None;
                self.status = ConnStatus::Connected;
                for event in events {
                    if matches!(event, ServerEvent::GameStarted { .. }) {
                        self.game_started = true;
                        self.ready_sent = false;
                    }
                    self.events.push(event);
                }
            }
            ServerMessage::PlayerConnectionChanged { seat, connected } => {
                if let Some(slot) = self.peer_connected.get_mut(seat) {
                    // 自分以外の座席のみ追跡する（自分は status で表す）
                    let is_self = self.room.as_ref().is_some_and(|r| r.your_seat == seat);
                    if !is_self {
                        *slot = Some(connected);
                    }
                }
            }
            ServerMessage::TurnTimer { seconds } => {
                self.turn_deadline = Some((self.clock)() + seconds as f64);
            }
            ServerMessage::GameOver { .. } => {
                self.game_over = true;
                self.reconnecting = false;
                self.reconnect_at = None;
            }
            ServerMessage::Error { code, message } => {
                if self.reconnecting && Self::is_terminal_reconnect_error(code) {
                    // 再入室できない種類のエラー: 再接続を断念する
                    self.reconnecting = false;
                    self.reconnect_at = None;
                    self.status = ConnStatus::Disconnected;
                }
                self.last_error = Some(RemoteError {
                    code: Some(code),
                    message,
                });
            }
        }
    }

    /// 接続中の他プレイヤーに切断者がいるか
    fn any_peer_disconnected(&self) -> bool {
        self.peer_connected.iter().any(|p| p == &Some(false))
    }

    /// 手番の制限時間の残り秒数（手番待ちでなければ None）
    pub fn turn_remaining_secs(&self) -> Option<u32> {
        self.turn_deadline.map(|deadline| {
            let remaining = (deadline - (self.clock)()).max(0.0);
            remaining.ceil() as u32
        })
    }

    fn send(&mut self, msg: &ClientMessage) {
        match msg.to_json() {
            Ok(json) => self.transport.send_text(&json),
            Err(e) => {
                self.last_error = Some(RemoteError {
                    code: None,
                    message: format!("メッセージの作成に失敗しました: {e}"),
                });
            }
        }
    }
}

impl GameAdapter for RemoteAdapter {
    fn send_action(&mut self, action: ClientAction) {
        // 操作したので手番のカウントダウンを止める
        self.turn_deadline = None;
        self.send(&ClientMessage::Action(action));
    }

    fn poll_events(&mut self) -> Vec<ServerEvent> {
        self.pump();
        std::mem::take(&mut self.events)
    }

    fn tick(&mut self) {
        self.pump();
    }

    fn request_next_round(&mut self) {
        // 多重クリックでの重複送信を防ぐ（次の GameStarted でリセット）
        if !self.ready_sent {
            self.send(&ClientMessage::ReadyNextRound);
            self.ready_sent = true;
        }
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn status_text(&self, lang: Lang) -> Option<String> {
        use crate::i18n::Key;
        if self.reconnecting {
            return Some(Key::Reconnecting.text(lang).to_string());
        }
        match self.status {
            ConnStatus::Disconnected => Some(Key::Disconnected.text(lang).to_string()),
            ConnStatus::Connecting => Some(Key::Connecting.text(lang).to_string()),
            ConnStatus::Connected => {
                if self.any_peer_disconnected() {
                    Some(Key::PeerDisconnected.text(lang).to_string())
                } else {
                    None
                }
            }
        }
    }

    fn turn_remaining_secs(&self) -> Option<u32> {
        // 固有メソッドへ委譲する（固有メソッドが優先解決されるため再帰しない）
        RemoteAdapter::turn_remaining_secs(self)
    }
}

/// 本番用の時計（macroquad の経過秒）
fn default_clock() -> Clock {
    Box::new(macroquad::time::get_time)
}

/// エラーコードを表示用の文言に変換する
pub fn error_code_message(code: ErrorCode, lang: Lang) -> &'static str {
    match lang {
        Lang::Ja => match code {
            ErrorCode::VersionMismatch => "クライアントのバージョンがサーバと一致しません",
            ErrorCode::RoomNotFound => "ルームが見つかりません",
            ErrorCode::RoomFull => "ルームが満席です",
            ErrorCode::NotHost => "ホストのみ操作できます",
            ErrorCode::NotInRoom => "ルームに参加していません",
            ErrorCode::GameInProgress => "対局中のため参加できません",
            ErrorCode::InvalidAction => "無効な操作です",
            ErrorCode::BadMessage => "不正なメッセージです",
            ErrorCode::RateLimited => "操作が頻繁すぎます。しばらく待ってください",
        },
        Lang::En => match code {
            ErrorCode::VersionMismatch => "Client version does not match the server",
            ErrorCode::RoomNotFound => "Room not found",
            ErrorCode::RoomFull => "Room is full",
            ErrorCode::NotHost => "Only the host can do that",
            ErrorCode::NotInRoom => "You are not in a room",
            ErrorCode::GameInProgress => "Cannot join: a game is in progress",
            ErrorCode::InvalidAction => "Invalid action",
            ErrorCode::BadMessage => "Malformed message",
            ErrorCode::RateLimited => "Too many actions; please wait a moment",
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use mahjong_core::tile::{Tile, Wind};

    use super::*;

    /// スクリプト化されたモックトランスポート
    struct MockTransport {
        incoming: Rc<RefCell<VecDeque<WsEvent>>>,
        sent: Rc<RefCell<Vec<String>>>,
    }

    /// モックの操作ハンドル（受信の注入と送信内容の検査）
    struct MockHandle {
        incoming: Rc<RefCell<VecDeque<WsEvent>>>,
        sent: Rc<RefCell<Vec<String>>>,
    }

    impl MockHandle {
        fn push(&self, event: WsEvent) {
            self.incoming.borrow_mut().push_back(event);
        }

        fn push_msg(&self, msg: &ServerMessage) {
            self.push(WsEvent::Message(msg.to_json().unwrap()));
        }

        fn sent(&self) -> Vec<ClientMessage> {
            self.sent
                .borrow()
                .iter()
                .map(|json| ClientMessage::from_json(json).unwrap())
                .collect()
        }
    }

    impl Transport for MockTransport {
        fn send_text(&mut self, text: &str) {
            self.sent.borrow_mut().push(text.to_string());
        }

        fn poll(&mut self) -> Vec<WsEvent> {
            self.incoming.borrow_mut().drain(..).collect()
        }
    }

    fn mock_pair() -> (Box<dyn Transport>, MockHandle) {
        let incoming = Rc::new(RefCell::new(VecDeque::new()));
        let sent = Rc::new(RefCell::new(Vec::new()));
        let transport = MockTransport {
            incoming: incoming.clone(),
            sent: sent.clone(),
        };
        (Box::new(transport), MockHandle { incoming, sent })
    }

    /// 再接続不要なテスト用のアダプターを作る
    ///
    /// コネクタは呼ばれない前提（呼ばれたら panic）、時計は常に 0 を返す。
    fn build_test(transport: Box<dyn Transport>, intent: LobbyIntent) -> RemoteAdapter {
        RemoteAdapter::build(
            transport,
            Box::new(|| panic!("このテストでは再接続を想定していません")),
            Box::new(|| 0.0),
            "テスト",
            intent,
        )
    }

    fn create_adapter() -> (RemoteAdapter, MockHandle) {
        let (transport, handle) = mock_pair();
        let adapter = build_test(transport, LobbyIntent::Create { round_count: 1 });
        (adapter, handle)
    }

    fn welcome() -> ServerMessage {
        ServerMessage::Welcome {
            session_token: "token123".to_string(),
            protocol_version: PROTOCOL_VERSION,
        }
    }

    fn room_state(your_seat: usize) -> ServerMessage {
        ServerMessage::RoomState {
            code: "ABC234".to_string(),
            seats: [
                SeatInfo::Human {
                    name: "ホスト".to_string(),
                    connected: true,
                },
                SeatInfo::Empty,
                SeatInfo::Empty,
                SeatInfo::Empty,
            ],
            host_seat: 0,
            your_seat,
        }
    }

    fn game_started_event() -> ServerEvent {
        ServerEvent::GameStarted {
            seat_wind: Wind::East,
            hand: vec![Tile::new(Tile::M1); 13],
            scores: [25000; 4],
            round_wind: Wind::East,
            dora_indicators: vec![Tile::new(Tile::P5)],
            round_number: 0,
            total_rounds: 4,
            honba: 0,
            riichi_sticks: 0,
        }
    }

    #[test]
    fn test_handshake_sends_hello_then_intent() {
        let (mut adapter, handle) = create_adapter();
        assert_eq!(adapter.status(), ConnStatus::Connecting);

        handle.push(WsEvent::Opened);
        adapter.tick();

        let sent = handle.sent();
        assert_eq!(sent.len(), 1);
        match &sent[0] {
            ClientMessage::Hello {
                protocol_version,
                display_name,
                ..
            } => {
                assert_eq!(*protocol_version, PROTOCOL_VERSION);
                assert_eq!(display_name, "テスト");
            }
            other => panic!("Helloでないメッセージ: {other:?}"),
        }

        handle.push_msg(&welcome());
        adapter.tick();

        assert_eq!(adapter.status(), ConnStatus::Connected);
        let sent = handle.sent();
        assert_eq!(sent.len(), 2);
        assert!(matches!(
            sent[1],
            ClientMessage::CreateRoom { round_count: 1 }
        ));
    }

    #[test]
    fn test_join_intent_uppercases_code() {
        let (transport, handle) = mock_pair();
        let mut adapter = build_test(
            transport,
            LobbyIntent::Join {
                code: " abc234 ".trim().to_ascii_uppercase(),
            },
        );

        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        adapter.tick();

        let sent = handle.sent();
        match &sent[1] {
            ClientMessage::JoinRoom { code } => assert_eq!(code, "ABC234"),
            other => panic!("JoinRoomでないメッセージ: {other:?}"),
        }
    }

    #[test]
    fn test_room_state_updates_room_view() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        handle.push_msg(&room_state(0));
        adapter.tick();

        let room = adapter.room().expect("ルーム情報が無い");
        assert_eq!(room.code, "ABC234");
        assert_eq!(room.your_seat, 0);
        assert!(room.is_host());
    }

    #[test]
    fn test_game_started_event_flows_through() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        handle.push_msg(&ServerMessage::Event(game_started_event()));

        let events = adapter.poll_events();
        assert!(adapter.game_started());
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ServerEvent::GameStarted { .. }));
        // 取り出したらキューは空になる
        assert!(adapter.poll_events().is_empty());
    }

    #[test]
    fn test_server_error_is_reported_once() {
        let (mut adapter, handle) = create_adapter();
        handle.push_msg(&ServerMessage::Error {
            code: ErrorCode::RoomNotFound,
            message: "no such room".to_string(),
        });
        adapter.tick();

        let err = adapter.take_error().expect("エラーが記録されていない");
        assert_eq!(err.code, Some(ErrorCode::RoomNotFound));
        assert!(adapter.take_error().is_none());
    }

    #[test]
    fn test_transport_error_disconnects() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Error("接続に失敗しました".to_string()));
        adapter.tick();

        assert_eq!(adapter.status(), ConnStatus::Disconnected);
        assert!(adapter.take_error().is_some());
        assert!(adapter.status_text(Lang::Ja).is_some());
    }

    #[test]
    fn test_closed_disconnects() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Closed);
        adapter.tick();
        assert_eq!(adapter.status(), ConnStatus::Disconnected);
    }

    #[test]
    fn test_send_action_serializes_action_message() {
        let (mut adapter, handle) = create_adapter();
        adapter.send_action(ClientAction::Discard { tile: None });

        let sent = handle.sent();
        assert!(matches!(
            sent[0],
            ClientMessage::Action(ClientAction::Discard { tile: None })
        ));
    }

    #[test]
    fn test_ready_next_round_is_deduplicated() {
        let (mut adapter, handle) = create_adapter();

        adapter.request_next_round();
        adapter.request_next_round();
        assert_eq!(
            handle
                .sent()
                .iter()
                .filter(|m| matches!(m, ClientMessage::ReadyNextRound))
                .count(),
            1
        );

        // 次の局が始まったら再送可能になる
        handle.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();
        adapter.request_next_round();
        assert_eq!(
            handle
                .sent()
                .iter()
                .filter(|m| matches!(m, ClientMessage::ReadyNextRound))
                .count(),
            2
        );
    }

    /// 実際のトランスポートでローカルサーバに接続し、1ゲーム打ち切るE2Eテスト
    ///
    /// 実行前に `cargo run -p mahjong-net-server` でサーバを起動しておくこと:
    /// `cargo test -p mahjong-client -- --ignored e2e`
    #[test]
    #[ignore = "要ローカルサーバ (cargo run -p mahjong-net-server)"]
    fn test_e2e_full_game_against_local_server() {
        let url = crate::transport::default_server_url();
        // 本番の時計は macroquad（ウィンドウ前提）なので、ヘッドレスな
        // E2E では std の経過秒で代用する
        let (transport, connector) = RemoteAdapter::connector_for(&url);
        let clock_start = std::time::Instant::now();
        let mut adapter = RemoteAdapter::build(
            transport,
            connector,
            Box::new(move || clock_start.elapsed().as_secs_f64()),
            "E2Eテスト",
            LobbyIntent::Create { round_count: 1 },
        );

        let start = std::time::Instant::now();
        let mut started = false;
        // 連続で届くイベント（TileDrawn + NineTerminalsAvailable など）を
        // まとめて判断するため、50msの静止を待ってから行動する
        let mut pending: Vec<ServerEvent> = Vec::new();
        let mut last_event_at = std::time::Instant::now();

        loop {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(120),
                "E2Eテストがタイムアウトした"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));

            adapter.tick();
            if let Some(err) = adapter.take_error() {
                panic!("サーバエラー: {:?} {}", err.code, err.message);
            }
            assert_ne!(
                adapter.status(),
                ConnStatus::Disconnected,
                "サーバから切断された"
            );
            if adapter.is_game_over() {
                break;
            }

            if !started {
                if adapter.room().is_some() {
                    adapter.start_game(None);
                    started = true;
                }
                continue;
            }

            let mut new_events = adapter.poll_events();
            if !new_events.is_empty() {
                pending.append(&mut new_events);
                last_event_at = std::time::Instant::now();
                continue;
            }
            if pending.is_empty() || last_event_at.elapsed() < std::time::Duration::from_millis(50)
            {
                continue;
            }

            let batch = std::mem::take(&mut pending);
            // 九種九牌の選択があるターンは宣言拒否のみ送る
            // （拒否するとサーバが TileDrawn を再送して打牌を促す）
            let nine_terminals = batch
                .iter()
                .any(|e| matches!(e, ServerEvent::NineTerminalsAvailable));
            if nine_terminals {
                adapter.send_action(ClientAction::NineTerminals { declare: false });
            }

            for event in batch {
                match event {
                    ServerEvent::TileDrawn { can_tsumo, .. } if !nine_terminals => {
                        let action = if can_tsumo {
                            ClientAction::Tsumo
                        } else {
                            ClientAction::Discard { tile: None }
                        };
                        adapter.send_action(action);
                    }
                    ServerEvent::CallAvailable { .. } => {
                        adapter.send_action(ClientAction::Pass);
                    }
                    ServerEvent::RoundWon { .. } | ServerEvent::RoundDraw { .. } => {
                        adapter.request_next_round();
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn test_game_over_sets_flag() {
        let (mut adapter, handle) = create_adapter();
        assert!(!adapter.is_game_over());
        handle.push_msg(&ServerMessage::GameOver {
            final_scores: [30000, 25000, 25000, 20000],
        });
        adapter.tick();
        assert!(adapter.is_game_over());
    }

    #[test]
    fn test_resync_replays_events_and_clears_reconnecting() {
        let (mut adapter, handle) = create_adapter();
        // 再接続中の状態を作る
        adapter.reconnecting = true;
        adapter.status = ConnStatus::Connecting;

        handle.push_msg(&ServerMessage::Resync {
            events: vec![
                game_started_event(),
                ServerEvent::TileDiscarded {
                    player: Wind::South,
                    tile: Tile::new(Tile::S9),
                    is_tsumogiri: true,
                },
            ],
        });

        let events = adapter.poll_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], ServerEvent::GameStarted { .. }));
        assert!(adapter.game_started());
        // 再接続が完了して通常状態へ戻る
        assert_eq!(adapter.status(), ConnStatus::Connected);
        assert!(adapter.status_text(Lang::Ja).is_none());
    }

    /// 連続するモックを払い出すコネクタを作る
    fn queued_connector(
        transports: Vec<Box<dyn Transport>>,
    ) -> Box<dyn FnMut() -> Box<dyn Transport>> {
        let mut queue = VecDeque::from(transports);
        Box::new(move || queue.pop_front().expect("コネクタが想定より多く呼ばれた"))
    }

    #[test]
    fn test_auto_reconnect_after_midgame_disconnect() {
        // 1本目のトランスポートで対局を開始し、2本目で再接続させる
        let (t1, h1) = mock_pair();
        let (t2, h2) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let mut adapter = RemoteAdapter::build(
            t1,
            queued_connector(vec![t2]),
            Box::new(move || *now_clock.borrow()),
            "テスト",
            LobbyIntent::Join {
                code: "ABC234".to_string(),
            },
        );

        // ハンドシェイク → ルーム入室 → 対局開始
        h1.push(WsEvent::Opened);
        h1.push_msg(&welcome());
        h1.push_msg(&room_state(1));
        h1.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();
        assert!(adapter.game_started());
        assert_eq!(adapter.status(), ConnStatus::Connected);

        // 対局中に切断: 再接続モードに入り、エラーは表に出さない
        h1.push(WsEvent::Closed);
        adapter.tick();
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("再接続中...")
        );
        assert!(adapter.take_error().is_none());

        // バックオフ前は再接続しない
        *now.borrow_mut() = 0.5;
        adapter.tick();
        assert!(h2.sent().is_empty());

        // バックオフ経過後に2本目で再接続: Hello を送る
        *now.borrow_mut() = 1.5;
        h2.push(WsEvent::Opened);
        adapter.tick();
        let sent = h2.sent();
        assert!(matches!(
            sent.first(),
            Some(ClientMessage::Hello {
                session_token: Some(_),
                ..
            })
        ));

        // Welcome → JoinRoom（保持していたルームコードで再入室）
        h2.push_msg(&welcome());
        adapter.tick();
        assert!(
            h2.sent()
                .iter()
                .any(|m| matches!(m, ClientMessage::JoinRoom { code } if code == "ABC234"))
        );

        // サーバが RoomState + Resync を返す → 再接続完了
        h2.push_msg(&room_state(1));
        h2.push_msg(&ServerMessage::Resync {
            events: vec![game_started_event()],
        });
        let events = adapter.poll_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::GameStarted { .. }))
        );
        assert_eq!(adapter.status(), ConnStatus::Connected);
        assert!(adapter.status_text(Lang::Ja).is_none());
    }

    #[test]
    fn test_reconnect_stops_on_terminal_error() {
        let (t1, h1) = mock_pair();
        let (t2, h2) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let mut adapter = RemoteAdapter::build(
            t1,
            queued_connector(vec![t2]),
            Box::new(move || *now_clock.borrow()),
            "テスト",
            LobbyIntent::Join {
                code: "ABC234".to_string(),
            },
        );

        h1.push(WsEvent::Opened);
        h1.push_msg(&welcome());
        h1.push_msg(&room_state(1));
        h1.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();

        h1.push(WsEvent::Closed);
        adapter.tick();

        // 再接続を試みるがルームが消えていた
        *now.borrow_mut() = 1.5;
        h2.push(WsEvent::Opened);
        h2.push_msg(&welcome());
        h2.push_msg(&ServerMessage::Error {
            code: ErrorCode::RoomNotFound,
            message: "room closed".to_string(),
        });
        adapter.tick();

        // 再接続を断念し、エラーと切断状態を表に出す
        assert_eq!(adapter.status(), ConnStatus::Disconnected);
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("サーバとの接続が切れました")
        );
        let err = adapter.take_error().expect("エラーが記録されていない");
        assert_eq!(err.code, Some(ErrorCode::RoomNotFound));
    }

    #[test]
    fn test_peer_disconnect_shows_status() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        // 自分は座席1、座席0（ホスト）が接続中
        handle.push_msg(&room_state(1));
        handle.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();
        assert!(adapter.status_text(Lang::Ja).is_none());

        // 座席0が切断 → 状態表示が出る
        handle.push_msg(&ServerMessage::PlayerConnectionChanged {
            seat: 0,
            connected: false,
        });
        adapter.tick();
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("他のプレイヤーが切断中（CPUが代打ち）")
        );

        // 座席0が再接続 → 表示が消える
        handle.push_msg(&ServerMessage::PlayerConnectionChanged {
            seat: 0,
            connected: true,
        });
        adapter.tick();
        assert!(adapter.status_text(Lang::Ja).is_none());
    }

    #[test]
    fn test_turn_timer_counts_down_and_clears_on_action() {
        let (transport, handle) = mock_pair();
        let now = Rc::new(RefCell::new(100.0_f64));
        let now_clock = now.clone();
        let mut adapter = RemoteAdapter::build(
            transport,
            Box::new(|| panic!("再接続なし")),
            Box::new(move || *now_clock.borrow()),
            "テスト",
            LobbyIntent::Create { round_count: 1 },
        );

        // 手番タイマー（90秒）を受信
        handle.push_msg(&ServerMessage::TurnTimer { seconds: 90 });
        adapter.tick();
        assert_eq!(adapter.turn_remaining_secs(), Some(90));

        // 30秒経過 → 残り60秒
        *now.borrow_mut() = 130.0;
        assert_eq!(adapter.turn_remaining_secs(), Some(60));

        // 操作するとカウントダウンが消える
        adapter.send_action(ClientAction::Discard { tile: None });
        assert_eq!(adapter.turn_remaining_secs(), None);
    }

    #[test]
    fn test_turn_timer_floors_at_zero() {
        let (transport, handle) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();
        let mut adapter = RemoteAdapter::build(
            transport,
            Box::new(|| panic!("再接続なし")),
            Box::new(move || *now_clock.borrow()),
            "テスト",
            LobbyIntent::Create { round_count: 1 },
        );
        handle.push_msg(&ServerMessage::TurnTimer { seconds: 5 });
        adapter.tick();
        // 期限を過ぎても負にならず 0
        *now.borrow_mut() = 100.0;
        assert_eq!(adapter.turn_remaining_secs(), Some(0));
    }

    #[test]
    fn test_lobby_disconnect_does_not_reconnect() {
        // 対局開始前の切断は通常エラー扱い（再接続しない）
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        handle.push_msg(&room_state(0));
        adapter.tick();

        handle.push(WsEvent::Error("接続失敗".to_string()));
        adapter.tick();

        assert_eq!(adapter.status(), ConnStatus::Disconnected);
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("サーバとの接続が切れました")
        );
    }
}
