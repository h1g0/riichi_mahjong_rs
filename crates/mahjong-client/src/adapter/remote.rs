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

use mahjong_server::protocol::net::{
    ClientMessage, ErrorCode, PROTOCOL_VERSION, SeatInfo, ServerMessage,
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

/// リモートアダプター: ネットワーク越しにサーバとやり取りする
pub struct RemoteAdapter {
    transport: Box<dyn Transport>,
    status: ConnStatus,
    display_name: String,
    session_token: Option<String>,
    pending_intent: Option<LobbyIntent>,
    room: Option<RoomView>,
    events: Vec<ServerEvent>,
    last_error: Option<RemoteError>,
    game_started: bool,
    game_over: bool,
    ready_sent: bool,
}

impl RemoteAdapter {
    /// トランスポートと意図を指定して作成する（テスト用にも使う）
    fn with_transport(
        transport: Box<dyn Transport>,
        display_name: &str,
        intent: LobbyIntent,
    ) -> Self {
        RemoteAdapter {
            transport,
            status: ConnStatus::Connecting,
            display_name: display_name.to_string(),
            session_token: None,
            pending_intent: Some(intent),
            room: None,
            events: Vec::new(),
            last_error: None,
            game_started: false,
            game_over: false,
            ready_sent: false,
        }
    }

    /// サーバに接続してルームを作成する
    pub fn create_room(url: &str, display_name: &str, round_count: u8) -> Self {
        Self::with_transport(
            crate::transport::connect(url),
            display_name,
            LobbyIntent::Create { round_count },
        )
    }

    /// サーバに接続して既存のルームに参加する
    pub fn join_room(url: &str, display_name: &str, code: &str) -> Self {
        Self::with_transport(
            crate::transport::connect(url),
            display_name,
            LobbyIntent::Join {
                code: code.trim().to_ascii_uppercase(),
            },
        )
    }

    /// 対局を開始する（ホストのみ有効。結果はサーバが判断する）
    pub fn start_game(&mut self) {
        self.send(&ClientMessage::StartGame);
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
                WsEvent::Closed => {
                    self.status = ConnStatus::Disconnected;
                }
                WsEvent::Error(message) => {
                    self.status = ConnStatus::Disconnected;
                    self.last_error = Some(RemoteError {
                        code: None,
                        message,
                    });
                }
            }
        }
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
            ServerMessage::GameOver { .. } => {
                self.game_over = true;
            }
            ServerMessage::Error { code, message } => {
                self.last_error = Some(RemoteError {
                    code: Some(code),
                    message,
                });
            }
            // 再接続（フェーズ5）で対応する
            ServerMessage::Resync { .. } | ServerMessage::PlayerConnectionChanged { .. } => {}
        }
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

    fn status_text(&self) -> Option<String> {
        match self.status {
            ConnStatus::Disconnected => Some("サーバとの接続が切れました".to_string()),
            ConnStatus::Connecting => Some("接続中...".to_string()),
            ConnStatus::Connected => None,
        }
    }
}

/// エラーコードを表示用の日本語文言に変換する
pub fn error_code_message(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::VersionMismatch => "クライアントのバージョンがサーバと一致しません",
        ErrorCode::RoomNotFound => "ルームが見つかりません",
        ErrorCode::RoomFull => "ルームが満席です",
        ErrorCode::NotHost => "ホストのみ操作できます",
        ErrorCode::NotInRoom => "ルームに参加していません",
        ErrorCode::GameInProgress => "対局中のため参加できません",
        ErrorCode::InvalidAction => "無効な操作です",
        ErrorCode::BadMessage => "不正なメッセージです",
        ErrorCode::RateLimited => "操作が頻繁すぎます。しばらく待ってください",
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

    fn create_adapter() -> (RemoteAdapter, MockHandle) {
        let (transport, handle) = mock_pair();
        let adapter = RemoteAdapter::with_transport(
            transport,
            "テスト",
            LobbyIntent::Create { round_count: 1 },
        );
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
            prevailing_wind: Wind::East,
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
        let mut adapter = RemoteAdapter::with_transport(
            transport,
            "ゲスト",
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
        assert!(adapter.status_text().is_some());
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
        let mut adapter = RemoteAdapter::create_room(&url, "E2Eテスト", 1);

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
                    adapter.start_game();
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
}
