//! Remote adapter: talks to mahjong-net-server over WebSocket, handling
//! lobby operations (create/join/start) and relaying in-game events.
//!
//! Connection and join flow:
//! 1. `create_room` / `join_room` opens the transport and stores the intent
//! 2. on `Opened`, send `Hello`
//! 3. on `Welcome`, send the stored `CreateRoom` / `JoinRoom`
//! 4. `RoomState` completes the join (`room()` becomes Some)
//! 5. the host calls `start_game`; `Event(GameStarted)` begins play

use mahjong_core::settings::{Lang, Settings};
use mahjong_server::protocol::net::{
    ClientMessage, CpuSpec, ErrorCode, PROTOCOL_VERSION, SeatInfo, ServerMessage,
};
use mahjong_server::protocol::{ClientAction, ServerEvent};
use mahjong_server::table::GameLength;

use super::GameAdapter;
use crate::transport::{Transport, WsEvent};

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnStatus {
    /// Connecting / handshaking
    Connecting,
    /// Connected
    Connected,
    /// Disconnected
    Disconnected,
}

/// Display snapshot of the room.
#[derive(Debug, Clone)]
pub struct RoomView {
    /// Room code
    pub code: String,
    /// Seat states
    pub seats: [SeatInfo; 4],
    /// The host's seat
    pub host_seat: usize,
    /// Our seat
    pub your_seat: usize,
    /// The room's rule settings
    pub rules: Settings,
    /// Game length
    pub length: GameLength,
    /// CPU configs for empty seats (None from old servers)
    pub cpu_configs: Option<[CpuSpec; 3]>,
    /// Whether the room is between completed games
    pub post_game: bool,
    /// Seats whose players have returned from the final results
    pub returned_to_lobby: [bool; 4],
}

impl RoomView {
    /// Whether we are the host.
    pub fn is_host(&self) -> bool {
        self.your_seat == self.host_seat
    }

    /// Whether this is a three-player room.
    pub fn three_player(&self) -> bool {
        self.rules.three_player
    }

    /// Player count (4 or 3).
    pub fn player_count(&self) -> usize {
        self.rules.player_count()
    }

    /// Whether every seated human is available for a rematch.
    pub fn can_start(&self) -> bool {
        !self.post_game
            || self
                .seats
                .iter()
                .enumerate()
                .take(self.player_count())
                .filter(|(_, seat)| matches!(seat, SeatInfo::Human { .. }))
                .all(|(seat, _)| self.returned_to_lobby[seat])
    }
}

/// The most recent error.
#[derive(Debug, Clone)]
pub struct RemoteError {
    /// The server's error code; None for transport errors
    pub code: Option<ErrorCode>,
    /// Technical detail for logs. User-facing text comes from `code` via
    /// [`error_code_message`], or a generic localized transport-error
    /// message when `code` is absent.
    pub message: String,
}

/// The lobby operation to send once Welcome arrives.
enum LobbyIntent {
    Create { length: GameLength, rules: Settings },
    Join { code: String },
}

/// Reconnect backoff steps in seconds, capped by attempt count.
const RECONNECT_BACKOFF: [f64; 5] = [1.0, 2.0, 4.0, 8.0, 10.0];
/// Maximum time for a reconnect handshake, including the resync.
const RECONNECT_HANDSHAKE_TIMEOUT: f64 = 15.0;

/// Factory for new transports, used on reconnection.
///
/// The argument is the room code to join or rejoin. When set, the URL
/// gains `?room=CODE` so a multi-machine server can forward the
/// connection to the room's owning machine before the upgrade.
type Connector = Box<dyn FnMut(Option<&str>) -> Box<dyn Transport>>;

/// Clock returning the current time in seconds.
type Clock = Box<dyn Fn() -> f64>;

/// Remote adapter: talks to the server over the network.
pub struct RemoteAdapter {
    transport: Box<dyn Transport>,
    /// Builds new transports for reconnection
    connector: Connector,
    /// Current time in seconds, for the reconnect backoff
    clock: Clock,
    status: ConnStatus,
    display_name: String,
    session_token: Option<String>,
    pending_intent: Option<LobbyIntent>,
    room: Option<RoomView>,
    /// Room code used to reconnect, learned from RoomState
    room_code: Option<String>,
    events: Vec<ServerEvent>,
    last_error: Option<RemoteError>,
    game_started: bool,
    game_over: bool,
    ready_sent: bool,
    /// Whether auto-reconnect is in progress
    reconnecting: bool,
    /// When the next reconnect attempt fires
    reconnect_at: Option<f64>,
    /// Deadline for the current reconnect handshake
    reconnect_timeout_at: Option<f64>,
    /// Reconnect attempt count
    reconnect_attempts: u32,
    /// Human players' connection state per seat (None = non-human/unknown)
    peer_connected: [Option<bool>; 4],
    /// Turn-timer deadline in clock seconds; None hides the countdown
    turn_deadline: Option<f64>,
}

impl RemoteAdapter {
    /// Creates the adapter from a transport, connector, and clock.
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
            reconnect_timeout_at: None,
            reconnect_attempts: 0,
            peer_connected: [None; 4],
            turn_deadline: None,
        }
    }

    /// Connects to the server and creates a room.
    ///
    /// `rules` carries the room's entire rule settings, so future rule
    /// pickers extend without changing this signature.
    pub fn create_room(url: &str, display_name: &str, length: GameLength, rules: Settings) -> Self {
        let mut connector = Self::connector_for(url);
        let transport = connector(None);
        Self::build(
            transport,
            connector,
            default_clock(),
            display_name,
            LobbyIntent::Create { length, rules },
        )
    }

    /// Connects to the server and joins an existing room.
    pub fn join_room(url: &str, display_name: &str, code: &str) -> Self {
        let code = code.trim().to_ascii_uppercase();
        let mut connector = Self::connector_for(url);
        // Attach the code from the start so even the first connection
        // reaches the owning machine.
        let transport = connector(Some(&code));
        Self::build(
            transport,
            connector,
            default_clock(),
            display_name,
            LobbyIntent::Join { code },
        )
    }

    /// Builds a connector for the given URL.
    fn connector_for(url: &str) -> Connector {
        let url = url.to_string();
        Box::new(move |room| match room {
            Some(code) => crate::transport::connect(&crate::transport::url_with_room(&url, code)),
            None => crate::transport::connect(&url),
        })
    }

    /// Starts the game (host only; the server validates).
    ///
    /// `cpu_configs` picks the CPU levels/personalities; `None` uses the
    /// server defaults.
    pub fn start_game(&mut self, cpu_configs: Option<[CpuSpec; 3]>) {
        self.send(&ClientMessage::StartGame { cpu_configs });
    }

    /// Configures the CPUs filling empty seats (host only). The server
    /// stores the configs and shares them via RoomState.
    pub fn set_cpu_configs(&mut self, cpu_configs: [CpuSpec; 3]) {
        self.send(&ClientMessage::SetCpuConfigs { cpu_configs });
    }

    /// Leaves the room.
    pub fn leave_room(&mut self) {
        self.send(&ClientMessage::LeaveRoom);
        self.room = None;
    }

    /// Returns the connection to the room after the final results.
    pub fn return_to_lobby(&mut self) {
        self.send(&ClientMessage::ReturnToLobby);
        self.game_started = false;
        self.game_over = false;
        self.ready_sent = false;
        self.events.clear();
        self.turn_deadline = None;
    }

    /// The current connection state.
    pub fn status(&self) -> ConnStatus {
        self.status
    }

    /// The joined room's info.
    pub fn room(&self) -> Option<&RoomView> {
        self.room.as_ref()
    }

    /// Whether the game has started (GameStarted received).
    pub fn game_started(&self) -> bool {
        self.game_started
    }

    /// Takes the most recent error, clearing it.
    pub fn take_error(&mut self) -> Option<RemoteError> {
        self.last_error.take()
    }

    /// Processes incoming traffic and updates internal state.
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
                            message: "invalid server message".to_string(),
                        });
                    }
                },
                WsEvent::Closed => self.handle_disconnect(None),
                WsEvent::Error(message) => self.handle_disconnect(Some(message)),
            }
        }
    }

    /// Handles disconnects and transport errors.
    ///
    /// Mid-game the adapter auto-reconnects, showing "reconnecting"
    /// rather than an error; in the lobby or after the game it is an
    /// ordinary disconnect.
    fn handle_disconnect(&mut self, message: Option<String>) {
        // Hide the turn countdown while disconnected; the server
        // re-sends it after the reconnect.
        self.turn_deadline = None;
        if self.should_auto_reconnect() {
            // A transient disconnect enters (or stays in) reconnect mode.
            if !self.reconnecting {
                self.enter_reconnect();
            } else {
                self.status = ConnStatus::Disconnected;
                self.reconnect_timeout_at = None;
                self.schedule_reconnect();
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

    /// Whether auto-reconnect applies (mid-game, not finished).
    fn should_auto_reconnect(&self) -> bool {
        self.game_started && !self.game_over && self.room_code.is_some()
    }

    /// Enters auto-reconnect mode.
    fn enter_reconnect(&mut self) {
        self.reconnecting = true;
        self.reconnect_attempts = 0;
        self.status = ConnStatus::Disconnected;
        self.reconnect_timeout_at = None;
        self.schedule_reconnect();
    }

    /// Schedules the next attempt after the backoff for its attempt number.
    fn schedule_reconnect(&mut self) {
        let idx = (self.reconnect_attempts as usize).min(RECONNECT_BACKOFF.len() - 1);
        self.reconnect_at = Some((self.clock)() + RECONNECT_BACKOFF[idx]);
    }

    /// Opens a new connection once the backoff expires.
    fn maybe_reconnect(&mut self) {
        if !self.reconnecting {
            return;
        }
        let now = (self.clock)();
        if self.reconnect_timeout_at.is_some_and(|at| now >= at) {
            // A transport may neither open nor report an error (for
            // example, a black-holed TCP connection). Give it a generous
            // deadline, then enter the normal backoff before replacing it.
            self.status = ConnStatus::Disconnected;
            self.reconnect_timeout_at = None;
            self.schedule_reconnect();
            return;
        }
        let Some(at) = self.reconnect_at else {
            return;
        };
        if now < at {
            return;
        }
        let Some(code) = self.room_code.clone() else {
            // Without a room code there is nothing to rejoin.
            self.reconnecting = false;
            self.reconnect_at = None;
            self.reconnect_timeout_at = None;
            return;
        };

        // Open a fresh transport and redo the join, attaching the room
        // code so multi-machine setups route back to the original room.
        self.transport = (self.connector)(Some(&code));
        self.status = ConnStatus::Connecting;
        self.pending_intent = Some(LobbyIntent::Join { code });
        self.reconnect_attempts = self.reconnect_attempts.saturating_add(1);
        // A new retry is scheduled only if this transport reports a
        // failure. Replacing an in-flight handshake after the next
        // backoff interval can prevent slow but healthy connections from
        // ever completing.
        self.reconnect_at = None;
        self.reconnect_timeout_at = Some(now + RECONNECT_HANDSHAKE_TIMEOUT);
    }

    /// Whether this error kind should abort reconnection.
    fn is_terminal_reconnect_error(code: ErrorCode) -> bool {
        matches!(
            code,
            ErrorCode::RoomNotFound | ErrorCode::NotInRoom | ErrorCode::VersionMismatch
        )
    }

    fn handle_server_message(&mut self, msg: ServerMessage) {
        // A subsequent successful message is evidence that an earlier
        // in-game protocol error is no longer the current status. Lobby
        // code normally consumes errors immediately via `take_error`.
        if !matches!(&msg, ServerMessage::Error { .. }) {
            self.last_error = None;
        }
        match msg {
            ServerMessage::Welcome { session_token, .. } => {
                self.session_token = Some(session_token);
                self.status = ConnStatus::Connected;
                if let Some(intent) = self.pending_intent.take() {
                    let msg = match intent {
                        LobbyIntent::Create { length, rules } => {
                            ClientMessage::CreateRoom { length, rules }
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
                rules,
                length,
                cpu_configs,
                post_game,
                returned_to_lobby,
            } => {
                self.room_code = Some(code.clone());
                // Pull the humans' connection states from the seats.
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
                    rules,
                    length,
                    cpu_configs,
                    post_game,
                    returned_to_lobby,
                });
            }
            ServerMessage::Event(event) => {
                if matches!(event, ServerEvent::GameStarted { .. }) {
                    self.game_started = true;
                    self.game_over = false;
                    // A new hand begins; the next-hand ack may be
                    // sent again.
                    self.ready_sent = false;
                }
                self.events.push(event);
            }
            ServerMessage::Resync { events } => {
                // Replay the current hand from the start; the reconnect
                // is complete, so return to the normal state.
                self.reconnecting = false;
                self.reconnect_at = None;
                self.reconnect_timeout_at = None;
                self.status = ConnStatus::Connected;
                for event in events {
                    if matches!(event, ServerEvent::GameStarted { .. }) {
                        self.game_started = true;
                        self.game_over = false;
                        self.ready_sent = false;
                    }
                    self.events.push(event);
                }
            }
            ServerMessage::PlayerConnectionChanged { seat, connected } => {
                if let Some(slot) = self.peer_connected.get_mut(seat) {
                    // Track other seats only; our own state is `status`.
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
                self.reconnect_timeout_at = None;
            }
            ServerMessage::Error { code, message } => {
                if self.reconnecting && code == ErrorCode::GameInProgress {
                    // The replacement socket can reach the room before the
                    // server observes the old socket closing. Retry instead
                    // of turning that ordering race into a permanent failure.
                    self.last_error = None;
                    self.status = ConnStatus::Disconnected;
                    self.reconnect_timeout_at = None;
                    self.schedule_reconnect();
                    return;
                }
                if self.reconnecting && Self::is_terminal_reconnect_error(code) {
                    // An unrejoinable error aborts the reconnection.
                    self.reconnecting = false;
                    self.reconnect_at = None;
                    self.reconnect_timeout_at = None;
                    self.status = ConnStatus::Disconnected;
                }
                self.last_error = Some(RemoteError {
                    code: Some(code),
                    message,
                });
            }
        }
    }

    /// Whether any other human is currently disconnected.
    fn any_peer_disconnected(&self) -> bool {
        self.peer_connected.iter().any(|p| p == &Some(false))
    }

    /// Seconds left on the turn timer; None when not awaited.
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
                    message: format!("failed to encode message: {e}"),
                });
            }
        }
    }
}

impl GameAdapter for RemoteAdapter {
    fn send_action(&mut self, action: ClientAction) {
        // Acting stops the turn countdown.
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
        // Guard against duplicate sends from double clicks;
        // reset by the next GameStarted.
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
        // Lobby errors are consumed by sync_online_ui; after promotion
        // to GameAdapter this is the only user-visible error channel.
        if self.game_started
            && let Some(error) = &self.last_error
        {
            let text = match error.code {
                Some(code) => error_code_message(code, lang),
                None => Key::NetworkError.text(lang),
            };
            return Some(text.to_string());
        }
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
        // Delegates to the inherent method; inherent resolution wins,
        // so this does not recurse.
        RemoteAdapter::turn_remaining_secs(self)
    }
}

/// Production clock: macroquad's elapsed seconds.
fn default_clock() -> Clock {
    Box::new(macroquad::time::get_time)
}

/// Maps an error code to display text.
pub fn error_code_message(code: ErrorCode, lang: Lang) -> &'static str {
    match lang {
        Lang::Ja => match code {
            ErrorCode::VersionMismatch => "クライアントのバージョンがサーバと一致しません",
            ErrorCode::RoomNotFound => "ルームが見つかりません",
            ErrorCode::RoomFull => "ルームが満席です",
            ErrorCode::NotHost => "ホストのみ操作できます",
            ErrorCode::NotInRoom => "ルームに参加していません",
            ErrorCode::GameInProgress => "対局中のため参加できません",
            ErrorCode::PlayersNotReady => "結果を確認中のプレイヤーがいます",
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
            ErrorCode::PlayersNotReady => "A player is still reviewing the results",
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

    /// A scripted mock transport.
    struct MockTransport {
        incoming: Rc<RefCell<VecDeque<WsEvent>>>,
        sent: Rc<RefCell<Vec<String>>>,
    }

    /// Mock handle for injecting received messages and inspecting sends.
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

    /// Builds a test adapter that never reconnects: the connector panics
    /// if called, the clock always returns 0.
    fn build_test(transport: Box<dyn Transport>, intent: LobbyIntent) -> RemoteAdapter {
        RemoteAdapter::build(
            transport,
            Box::new(|_| panic!("this test does not expect a reconnection attempt")),
            Box::new(|| 0.0),
            "Test",
            intent,
        )
    }

    fn create_adapter() -> (RemoteAdapter, MockHandle) {
        let (transport, handle) = mock_pair();
        let adapter = build_test(
            transport,
            LobbyIntent::Create {
                length: GameLength::EastOnly,
                rules: Settings::new(),
            },
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
                    name: "Host".to_string(),
                    connected: true,
                },
                SeatInfo::Empty,
                SeatInfo::Empty,
                SeatInfo::Empty,
            ],
            host_seat: 0,
            your_seat,
            rules: Settings::new(),
            length: GameLength::EastOnly,
            cpu_configs: None,
            post_game: false,
            returned_to_lobby: [false; 4],
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
            three_player: false,
            nuki_dora: false,
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
                assert_eq!(display_name, "Test");
            }
            other => panic!("expected Hello message, got {other:?}"),
        }

        handle.push_msg(&welcome());
        adapter.tick();

        assert_eq!(adapter.status(), ConnStatus::Connected);
        let sent = handle.sent();
        assert_eq!(sent.len(), 2);
        match &sent[1] {
            ClientMessage::CreateRoom { length, rules } => {
                assert_eq!(*length, GameLength::EastOnly);
                assert_eq!(*rules, Settings::new());
            }
            other => panic!("expected CreateRoom message, got {other:?}"),
        }
    }

    /// Hanchan settings must be sent in CreateRoom and RoomState's length
    /// must reach the lobby's RoomView (#271).
    #[test]
    fn test_hanchan_length_roundtrip() {
        let (transport, handle) = mock_pair();
        let mut adapter = build_test(
            transport,
            LobbyIntent::Create {
                length: GameLength::Hanchan,
                rules: Settings::new(),
            },
        );

        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        adapter.tick();

        let sent = handle.sent();
        match &sent[1] {
            ClientMessage::CreateRoom { length, .. } => assert_eq!(*length, GameLength::Hanchan),
            other => panic!("expected CreateRoom message, got {other:?}"),
        }

        let msg = ServerMessage::RoomState {
            code: "ABC234".to_string(),
            seats: [
                SeatInfo::Human {
                    name: "Host".to_string(),
                    connected: true,
                },
                SeatInfo::Empty,
                SeatInfo::Empty,
                SeatInfo::Empty,
            ],
            host_seat: 0,
            your_seat: 0,
            rules: Settings::new(),
            length: GameLength::Hanchan,
            cpu_configs: None,
            post_game: false,
            returned_to_lobby: [false; 4],
        };
        handle.push_msg(&msg);
        adapter.tick();

        let room = adapter.room().expect("adapter should be in a room");
        assert_eq!(room.length, GameLength::Hanchan);
        assert!(!room.three_player());
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
            other => panic!("expected JoinRoom message, got {other:?}"),
        }
    }

    #[test]
    fn test_room_state_updates_room_view() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        handle.push_msg(&room_state(0));
        adapter.tick();

        let room = adapter.room().expect("room information is missing");
        assert_eq!(room.code, "ABC234");
        assert_eq!(room.your_seat, 0);
        assert!(room.is_host());
    }

    /// set_cpu_configs must send SetCpuConfigs, and RoomState's CPU
    /// configs must reach the RoomView (#245).
    #[test]
    fn test_set_cpu_configs_roundtrip() {
        use mahjong_server::cpu::client::{CpuLevel, CpuPersonality};

        let (mut adapter, handle) = create_adapter();
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
        adapter.set_cpu_configs(specs);

        let sent = handle.sent();
        assert!(matches!(
            sent[0],
            ClientMessage::SetCpuConfigs { cpu_configs } if cpu_configs == specs
        ));

        let msg = ServerMessage::RoomState {
            code: "ABC234".to_string(),
            seats: [
                SeatInfo::Human {
                    name: "Host".to_string(),
                    connected: true,
                },
                SeatInfo::Empty,
                SeatInfo::Empty,
                SeatInfo::Empty,
            ],
            host_seat: 0,
            your_seat: 0,
            rules: Settings::new(),
            length: GameLength::EastOnly,
            cpu_configs: Some(specs),
            post_game: false,
            returned_to_lobby: [false; 4],
        };
        handle.push_msg(&msg);
        adapter.tick();

        let room = adapter.room().expect("adapter should be in a room");
        assert_eq!(room.cpu_configs, Some(specs));
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
        // Taking the error clears it.
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

        let err = adapter.take_error().expect("error was not recorded");
        assert_eq!(err.code, Some(ErrorCode::RoomNotFound));
        assert!(adapter.take_error().is_none());
    }

    #[test]
    fn test_in_game_server_error_is_visible_until_recovery() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        handle.push_msg(&room_state(1));
        handle.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();
        assert!(adapter.status_text(Lang::En).is_none());

        handle.push_msg(&ServerMessage::Error {
            code: ErrorCode::InvalidAction,
            message: "rejected".to_string(),
        });
        adapter.tick();
        assert_eq!(
            adapter.status_text(Lang::En).as_deref(),
            Some("Invalid action")
        );

        handle.push_msg(&ServerMessage::TurnTimer { seconds: 30 });
        adapter.tick();
        assert!(adapter.status_text(Lang::En).is_none());
    }

    #[test]
    fn test_transport_error_disconnects() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Error("connection failed".to_string()));
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

        // The next hand re-enables sending.
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

    /// E2E test: connects to a local server over the real transport and
    /// plays a game to the end.
    ///
    /// Start the server first with `cargo run -p mahjong-net-server`:
    /// `cargo test -p mahjong-client -- --ignored e2e`
    #[test]
    #[ignore = "requires a local server (cargo run -p mahjong-net-server)"]
    fn test_e2e_full_game_against_local_server() {
        let url = crate::transport::default_server_url();
        // The production clock needs a macroquad window; headless E2E
        // substitutes std elapsed seconds.
        let mut connector = RemoteAdapter::connector_for(&url);
        let transport = connector(None);
        let clock_start = std::time::Instant::now();
        let mut adapter = RemoteAdapter::build(
            transport,
            connector,
            Box::new(move || clock_start.elapsed().as_secs_f64()),
            "E2E Test",
            LobbyIntent::Create {
                length: GameLength::EastOnly,
                rules: Settings::new(),
            },
        );

        let start = std::time::Instant::now();
        let mut started = false;
        // Wait for 50ms of quiet before acting, so event bursts
        // (TileDrawn + NineTerminalsAvailable etc.) are judged together.
        let mut pending: Vec<ServerEvent> = Vec::new();
        let mut last_event_at = std::time::Instant::now();

        loop {
            // Online CPU pacing makes a healthy full game exceed two minutes
            // (#229). Detect stalls separately from the overall safety limit.
            assert!(
                last_event_at.elapsed() < std::time::Duration::from_secs(30),
                "E2E test received no game events for 30 seconds"
            );
            assert!(
                start.elapsed() < std::time::Duration::from_secs(15 * 60),
                "E2E game exceeded the 15-minute safety limit"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));

            adapter.tick();
            if let Some(err) = adapter.take_error() {
                panic!("server error: {:?} {}", err.code, err.message);
            }
            assert_ne!(
                adapter.status(),
                ConnStatus::Disconnected,
                "disconnected from the server"
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
            // Decline any nine-terminals offer; the server re-sends
            // TileDrawn to prompt the discard.
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
    fn test_return_to_lobby_sends_intent_and_resets_game_flags() {
        let (mut adapter, handle) = create_adapter();
        handle.push_msg(&ServerMessage::Event(game_started_event()));
        handle.push_msg(&ServerMessage::GameOver {
            final_scores: [30000, 25000, 25000, 20000],
        });
        adapter.tick();
        assert!(adapter.game_started());
        assert!(adapter.is_game_over());

        adapter.return_to_lobby();

        assert!(!adapter.game_started());
        assert!(!adapter.is_game_over());
        assert!(matches!(
            handle.sent().last(),
            Some(ClientMessage::ReturnToLobby)
        ));
    }

    #[test]
    fn test_post_game_room_waits_for_every_seated_human() {
        let (mut adapter, handle) = create_adapter();
        handle.push_msg(&ServerMessage::RoomState {
            code: "ABC234".to_string(),
            seats: [
                SeatInfo::Human {
                    name: "Host".to_string(),
                    connected: true,
                },
                SeatInfo::Human {
                    name: "Guest".to_string(),
                    connected: true,
                },
                SeatInfo::Empty,
                SeatInfo::Empty,
            ],
            host_seat: 0,
            your_seat: 0,
            rules: Settings::new(),
            length: GameLength::EastOnly,
            cpu_configs: None,
            post_game: true,
            returned_to_lobby: [true, false, false, false],
        });
        adapter.tick();
        assert!(!adapter.room().expect("room").can_start());

        handle.push_msg(&ServerMessage::RoomState {
            code: "ABC234".to_string(),
            seats: [
                SeatInfo::Human {
                    name: "Host".to_string(),
                    connected: true,
                },
                SeatInfo::Human {
                    name: "Guest".to_string(),
                    connected: true,
                },
                SeatInfo::Empty,
                SeatInfo::Empty,
            ],
            host_seat: 0,
            your_seat: 0,
            rules: Settings::new(),
            length: GameLength::EastOnly,
            cpu_configs: None,
            post_game: true,
            returned_to_lobby: [true, true, false, false],
        });
        adapter.tick();
        assert!(adapter.room().expect("room").can_start());
    }

    #[test]
    fn test_resync_replays_events_and_clears_reconnecting() {
        let (mut adapter, handle) = create_adapter();
        adapter.reconnecting = true;
        adapter.status = ConnStatus::Connecting;

        handle.push_msg(&ServerMessage::Resync {
            events: vec![
                game_started_event(),
                ServerEvent::TileDiscarded {
                    player: Wind::South,
                    tile: Tile::new(Tile::S9),
                    is_tsumogiri: true,
                    hand_index: None,
                },
            ],
        });

        let events = adapter.poll_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], ServerEvent::GameStarted { .. }));
        assert!(adapter.game_started());
        // The reconnect completes and the state returns to normal.
        assert_eq!(adapter.status(), ConnStatus::Connected);
        assert!(adapter.status_text(Lang::Ja).is_none());
    }

    /// Builds a connector yielding successive mocks and recording the
    /// room codes it was given.
    fn queued_connector(
        transports: Vec<Box<dyn Transport>>,
    ) -> (Connector, Rc<RefCell<Vec<Option<String>>>>) {
        let mut queue = VecDeque::from(transports);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let record = calls.clone();
        let connector = Box::new(move |room: Option<&str>| {
            record.borrow_mut().push(room.map(String::from));
            queue
                .pop_front()
                .expect("connector was called more times than expected")
        });
        (connector, calls)
    }

    #[test]
    fn test_auto_reconnect_after_midgame_disconnect() {
        // Start the game on the first transport, reconnect on the second.
        let (t1, h1) = mock_pair();
        let (t2, h2) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let (connector, connector_calls) = queued_connector(vec![t2]);
        let mut adapter = RemoteAdapter::build(
            t1,
            connector,
            Box::new(move || *now_clock.borrow()),
            "Test",
            LobbyIntent::Join {
                code: "ABC234".to_string(),
            },
        );

        h1.push(WsEvent::Opened);
        h1.push_msg(&welcome());
        h1.push_msg(&room_state(1));
        h1.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();
        assert!(adapter.game_started());
        assert_eq!(adapter.status(), ConnStatus::Connected);

        // A mid-game disconnect enters reconnect mode with no
        // visible error.
        h1.push(WsEvent::Closed);
        adapter.tick();
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("再接続中...")
        );
        assert!(adapter.take_error().is_none());

        // No reconnect before the backoff expires.
        *now.borrow_mut() = 0.5;
        adapter.tick();
        assert!(h2.sent().is_empty());

        // Past the backoff the second transport reconnects and sends
        // Hello; the room code rides along for multi-machine routing.
        *now.borrow_mut() = 1.5;
        h2.push(WsEvent::Opened);
        adapter.tick();
        assert_eq!(
            connector_calls.borrow().as_slice(),
            [Some("ABC234".to_string())]
        );
        let sent = h2.sent();
        assert!(matches!(
            sent.first(),
            Some(ClientMessage::Hello {
                session_token: Some(_),
                ..
            })
        ));

        // Welcome triggers a JoinRoom with the stored room code.
        h2.push_msg(&welcome());
        adapter.tick();
        assert!(
            h2.sent()
                .iter()
                .any(|m| matches!(m, ClientMessage::JoinRoom { code } if code == "ABC234"))
        );

        // RoomState + Resync complete the reconnection.
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
    fn test_hung_reconnect_uses_handshake_timeout_then_backoff() {
        let (t1, h1) = mock_pair();
        let (t2, _h2) = mock_pair();
        let (t3, h3) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let (connector, connector_calls) = queued_connector(vec![t2, t3]);
        let mut adapter = RemoteAdapter::build(
            t1,
            connector,
            Box::new(move || *now_clock.borrow()),
            "Test",
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
        *now.borrow_mut() = 1.5;
        adapter.tick();
        assert_eq!(adapter.status(), ConnStatus::Connecting);
        assert_eq!(connector_calls.borrow().len(), 1);

        // The ordinary two-second backoff must not replace a slow
        // in-flight handshake.
        *now.borrow_mut() = 16.4;
        adapter.tick();
        assert_eq!(adapter.status(), ConnStatus::Connecting);
        assert_eq!(connector_calls.borrow().len(), 1);

        // Once the explicit handshake deadline expires, the next retry
        // still waits for its normal backoff.
        *now.borrow_mut() = 16.6;
        adapter.tick();
        assert_eq!(adapter.status(), ConnStatus::Disconnected);
        assert_eq!(connector_calls.borrow().len(), 1);
        *now.borrow_mut() = 18.5;
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 1);

        *now.borrow_mut() = 18.7;
        h3.push(WsEvent::Opened);
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 2);
        assert!(matches!(
            h3.sent().first(),
            Some(ClientMessage::Hello { .. })
        ));
    }

    #[test]
    fn test_failed_reconnect_schedules_the_next_backoff() {
        let (t1, h1) = mock_pair();
        let (t2, h2) = mock_pair();
        let (t3, h3) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let (connector, connector_calls) = queued_connector(vec![t2, t3]);
        let mut adapter = RemoteAdapter::build(
            t1,
            connector,
            Box::new(move || *now_clock.borrow()),
            "Test",
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

        // The first retry fails, so the second step (two seconds) starts
        // from the failure time.
        *now.borrow_mut() = 1.5;
        h2.push(WsEvent::Error("retry failed".to_string()));
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 1);
        *now.borrow_mut() = 3.4;
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 1);

        *now.borrow_mut() = 3.6;
        h3.push(WsEvent::Opened);
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 2);
        assert!(matches!(
            h3.sent().first(),
            Some(ClientMessage::Hello { .. })
        ));
    }

    #[test]
    fn test_reconnect_stops_on_terminal_error() {
        let (t1, h1) = mock_pair();
        let (t2, h2) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let (connector, _connector_calls) = queued_connector(vec![t2]);
        let mut adapter = RemoteAdapter::build(
            t1,
            connector,
            Box::new(move || *now_clock.borrow()),
            "Test",
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

        // The reconnect finds the room gone.
        *now.borrow_mut() = 1.5;
        h2.push(WsEvent::Opened);
        h2.push_msg(&welcome());
        h2.push_msg(&ServerMessage::Error {
            code: ErrorCode::RoomNotFound,
            message: "room closed".to_string(),
        });
        adapter.tick();

        // Reconnection aborts, surfacing the error and disconnect.
        assert_eq!(adapter.status(), ConnStatus::Disconnected);
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("ルームが見つかりません")
        );
        let err = adapter.take_error().expect("error was not recorded");
        assert_eq!(err.code, Some(ErrorCode::RoomNotFound));
    }

    #[test]
    fn test_reconnect_retries_game_in_progress_race() {
        let (t1, h1) = mock_pair();
        let (t2, h2) = mock_pair();
        let (t3, h3) = mock_pair();
        let now = Rc::new(RefCell::new(0.0_f64));
        let now_clock = now.clone();

        let (connector, connector_calls) = queued_connector(vec![t2, t3]);
        let mut adapter = RemoteAdapter::build(
            t1,
            connector,
            Box::new(move || *now_clock.borrow()),
            "Test",
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

        // The server has not processed the old socket's close yet.
        *now.borrow_mut() = 1.5;
        h2.push(WsEvent::Opened);
        h2.push_msg(&welcome());
        h2.push_msg(&ServerMessage::Error {
            code: ErrorCode::GameInProgress,
            message: "old connection still active".to_string(),
        });
        adapter.tick();

        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("再接続中...")
        );
        assert!(adapter.take_error().is_none());
        assert_eq!(connector_calls.borrow().len(), 1);

        // GameInProgress uses the next normal backoff and tries again.
        *now.borrow_mut() = 3.4;
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 1);
        *now.borrow_mut() = 3.6;
        h3.push(WsEvent::Opened);
        adapter.tick();
        assert_eq!(connector_calls.borrow().len(), 2);
        assert!(matches!(
            h3.sent().first(),
            Some(ClientMessage::Hello { .. })
        ));
    }

    #[test]
    fn test_peer_disconnect_shows_status() {
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        // We sit at seat 1; the host at seat 0 is connected.
        handle.push_msg(&room_state(1));
        handle.push_msg(&ServerMessage::Event(game_started_event()));
        adapter.tick();
        assert!(adapter.status_text(Lang::Ja).is_none());

        // Seat 0 disconnecting shows the notice.
        handle.push_msg(&ServerMessage::PlayerConnectionChanged {
            seat: 0,
            connected: false,
        });
        adapter.tick();
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("他のプレイヤーが切断中（CPUが代打ち）")
        );

        // Seat 0 reconnecting clears it.
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
            Box::new(|_| panic!("unexpected reconnection attempt")),
            Box::new(move || *now_clock.borrow()),
            "Test",
            LobbyIntent::Create {
                length: GameLength::EastOnly,
                rules: Settings::new(),
            },
        );

        handle.push_msg(&ServerMessage::TurnTimer { seconds: 90 });
        adapter.tick();
        assert_eq!(adapter.turn_remaining_secs(), Some(90));

        *now.borrow_mut() = 130.0;
        assert_eq!(adapter.turn_remaining_secs(), Some(60));

        // Acting clears the countdown.
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
            Box::new(|_| panic!("unexpected reconnection attempt")),
            Box::new(move || *now_clock.borrow()),
            "Test",
            LobbyIntent::Create {
                length: GameLength::EastOnly,
                rules: Settings::new(),
            },
        );
        handle.push_msg(&ServerMessage::TurnTimer { seconds: 5 });
        adapter.tick();
        // Past the deadline the remainder clamps to 0.
        *now.borrow_mut() = 100.0;
        assert_eq!(adapter.turn_remaining_secs(), Some(0));
    }

    #[test]
    fn test_lobby_disconnect_does_not_reconnect() {
        // A pre-game disconnect is an ordinary error; no reconnect.
        let (mut adapter, handle) = create_adapter();
        handle.push(WsEvent::Opened);
        handle.push_msg(&welcome());
        handle.push_msg(&room_state(0));
        adapter.tick();

        handle.push(WsEvent::Error("connection failed".to_string()));
        adapter.tick();

        assert_eq!(adapter.status(), ConnStatus::Disconnected);
        assert_eq!(
            adapter.status_text(Lang::Ja).as_deref(),
            Some("サーバとの接続が切れました")
        );
    }
}
