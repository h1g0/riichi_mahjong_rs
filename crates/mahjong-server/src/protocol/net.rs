//! Network messages for online play.
//!
//! Envelope types exchanged as JSON over WebSocket text frames. In-game
//! traffic wraps the existing `ClientAction` / `ServerEvent` unchanged.

use mahjong_core::settings::Settings;
use serde::{Deserialize, Serialize};

use super::{ClientAction, ServerEvent};
use crate::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use crate::table::GameLength;

/// Protocol version.
///
/// Incremented on incompatible changes. Checked in `Hello`; a mismatch
/// disconnects with `ErrorCode::VersionMismatch`.
/// v3: three-player support (CreateRoom / RoomState carry the whole
/// `Settings`).
/// v4: structured Nagashi Mangan round-end events.
/// v5: double-yakuman settings and dedicated special-yakuman score items.
pub const PROTOCOL_VERSION: u32 = 5;

/// A CPU's level and personality.
///
/// Sent by the host at game start; the server assigns them to empty seats
/// and shadow CPUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSpec {
    pub level: CpuLevel,
    pub personality: CpuPersonality,
}

impl CpuSpec {
    pub fn to_config(self) -> CpuConfig {
        CpuConfig::new(self.level, self.personality)
    }

    pub fn from_config(config: &CpuConfig) -> Self {
        CpuSpec {
            level: config.level,
            personality: config.personality,
        }
    }
}

/// Messages from a client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Connection handshake; must be the very first message
    Hello {
        /// The client's protocol version
        protocol_version: u32,
        /// Session token presented on reconnection
        session_token: Option<String>,
        /// Display name
        display_name: String,
    },

    /// Create a room; the creator becomes the host
    CreateRoom {
        /// Game length (East-only or hanchan)
        #[serde(default)]
        length: GameLength,
        /// Rule settings (all flags: three-player, pei dora, kuitan,
        /// triple ron, ...)
        ///
        /// Starting scores are derived server-side from the rules
        /// (25000 four-player, 35000 three-player). Missing fields fall
        /// back to defaults, so new rule flags stay compatible with old
        /// clients' messages.
        #[serde(default)]
        rules: Settings,
    },

    /// Join a room by code
    JoinRoom {
        /// Six-character room code
        code: String,
    },

    /// Leave the room
    LeaveRoom,

    /// Configure the CPUs that fill empty seats (host only, pre-game)
    ///
    /// The server stores the configs and shares them via `RoomState`.
    /// Configs passed to `StartGame` take precedence at game start.
    SetCpuConfigs {
        /// CPU configs in seat order: right, across, left
        cpu_configs: [CpuSpec; 3],
    },

    /// Start the game (host only; CPUs fill the empty seats)
    StartGame {
        /// Host-chosen CPU configs in seat order (right, across, left);
        /// `None` uses the server defaults
        cpu_configs: Option<[CpuSpec; 3]>,
    },

    /// An in-game action
    Action(ClientAction),

    /// The player has reviewed the result screen and is ready for
    /// the next hand
    ReadyNextRound,
}

/// Messages from the server to a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Handshake response
    Welcome {
        /// Session token for reconnection
        session_token: String,
        /// The server's protocol version
        protocol_version: u32,
    },

    /// Room state, broadcast on joins, leaves, and connection changes
    RoomState {
        /// Room code
        code: String,
        /// Seat states in seat order (seat 3 is always empty in
        /// three-player games)
        seats: [SeatInfo; 4],
        /// The host's seat index
        host_seat: usize,
        /// The recipient's own seat index
        your_seat: usize,
        /// The room's rule settings (used e.g. to show three-player mode)
        #[serde(default)]
        rules: Settings,
        /// Game length (East-only or hanchan), for the lobby display
        #[serde(default)]
        length: GameLength,
        /// CPU configs for empty seats in seat order (right, across, left)
        ///
        /// Changed by the host via `SetCpuConfigs`; drives the lobby's
        /// empty-seat display. Old servers omit it, so it falls back
        /// to `None`.
        #[serde(default)]
        cpu_configs: Option<[CpuSpec; 3]>,
    },

    /// An in-game event
    Event(ServerEvent),

    /// State resync on reconnection: replays the current hand's events
    Resync {
        /// Events since the current hand's `GameStarted`
        events: Vec<ServerEvent>,
    },

    /// The game is over
    GameOver {
        /// Final scores
        final_scores: [i32; 4],
    },

    /// A player's connection state changed
    PlayerConnectionChanged {
        /// Seat index
        seat: usize,
        /// Whether the player is connected
        connected: bool,
    },

    /// Turn time limit, sent to the seat whose action is awaited
    ///
    /// If the player does not act in time the server performs the default
    /// action (tsumogiri / pass) on their behalf. Clients show a countdown
    /// from this value.
    TurnTimer {
        /// Time limit in seconds
        seconds: u32,
    },

    /// An error notification
    Error {
        /// Error code
        code: ErrorCode,
        /// Supplementary debug message; user-facing text is built
        /// client-side
        message: String,
    },
}

/// State of one seat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeatInfo {
    Empty,
    Cpu {
        level: CpuLevel,
        personality: CpuPersonality,
    },
    Human {
        /// Display name
        name: String,
        /// Whether the player is connected
        connected: bool,
    },
}

/// Error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    VersionMismatch,
    RoomNotFound,
    RoomFull,
    /// Host-only operation attempted by a non-host
    NotHost,
    NotInRoom,
    /// Rejected because a game is in progress
    GameInProgress,
    /// Invalid action (wrong turn, wrong phase, ...)
    InvalidAction,
    /// Unparseable message
    BadMessage,
    RateLimited,
}

impl ClientMessage {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl ServerMessage {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::{Tile, Wind};

    fn roundtrip_client(msg: ClientMessage) -> ClientMessage {
        let json = msg.to_json().expect("encode");
        ClientMessage::from_json(&json).expect("decode")
    }

    fn roundtrip_server(msg: ServerMessage) -> ServerMessage {
        let json = msg.to_json().expect("encode");
        ServerMessage::from_json(&json).expect("decode")
    }

    #[test]
    fn test_client_message_roundtrip_all_variants() {
        let messages = vec![
            ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                session_token: Some("abc123".to_string()),
                display_name: "テスト".to_string(),
            },
            ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                session_token: None,
                display_name: String::new(),
            },
            ClientMessage::CreateRoom {
                length: GameLength::Hanchan,
                rules: Settings::new(),
            },
            ClientMessage::CreateRoom {
                length: GameLength::EastOnly,
                rules: Settings {
                    three_player: true,
                    nuki_dora: false,
                    triple_ron_draw: true,
                    double_yakuman: false,
                    ..Settings::new()
                },
            },
            ClientMessage::JoinRoom {
                code: "ABC234".to_string(),
            },
            ClientMessage::LeaveRoom,
            ClientMessage::SetCpuConfigs {
                cpu_configs: [
                    CpuSpec {
                        level: CpuLevel::Weak,
                        personality: CpuPersonality::Defensive,
                    },
                    CpuSpec {
                        level: CpuLevel::Normal,
                        personality: CpuPersonality::Balanced,
                    },
                    CpuSpec {
                        level: CpuLevel::Strong,
                        personality: CpuPersonality::Speedy,
                    },
                ],
            },
            ClientMessage::StartGame { cpu_configs: None },
            ClientMessage::StartGame {
                cpu_configs: Some([
                    CpuSpec {
                        level: CpuLevel::Weak,
                        personality: CpuPersonality::Balanced,
                    },
                    CpuSpec {
                        level: CpuLevel::Normal,
                        personality: CpuPersonality::Speedy,
                    },
                    CpuSpec {
                        level: CpuLevel::Strong,
                        personality: CpuPersonality::HighValue,
                    },
                ]),
            },
            ClientMessage::Action(ClientAction::Discard {
                tile: Some(Tile::new(Tile::M1)),
            }),
            ClientMessage::Action(ClientAction::Riichi { tile: None }),
            ClientMessage::ReadyNextRound,
        ];

        for msg in messages {
            let decoded = roundtrip_client(msg.clone());
            assert_eq!(
                format!("{:?}", decoded),
                format!("{:?}", msg),
                "round-trip mismatch"
            );
        }
    }

    #[test]
    fn test_server_message_roundtrip_all_variants() {
        let messages = vec![
            ServerMessage::Welcome {
                session_token: "deadbeef".to_string(),
                protocol_version: PROTOCOL_VERSION,
            },
            ServerMessage::RoomState {
                code: "XYZ789".to_string(),
                seats: [
                    SeatInfo::Human {
                        name: "ホスト".to_string(),
                        connected: true,
                    },
                    SeatInfo::Human {
                        name: "ゲスト".to_string(),
                        connected: false,
                    },
                    SeatInfo::Cpu {
                        level: CpuLevel::Normal,
                        personality: CpuPersonality::Speedy,
                    },
                    SeatInfo::Empty,
                ],
                host_seat: 0,
                your_seat: 1,
                rules: Settings::new(),
                length: GameLength::Hanchan,
                cpu_configs: Some([
                    CpuSpec {
                        level: CpuLevel::Normal,
                        personality: CpuPersonality::Balanced,
                    },
                    CpuSpec {
                        level: CpuLevel::Normal,
                        personality: CpuPersonality::Speedy,
                    },
                    CpuSpec {
                        level: CpuLevel::Normal,
                        personality: CpuPersonality::HighValue,
                    },
                ]),
            },
            ServerMessage::Event(ServerEvent::TileDrawn {
                tile: Tile::new(Tile::P5),
                remaining_tiles: 69,
                can_tsumo: false,
                can_riichi: true,
                is_furiten: false,
            }),
            ServerMessage::Event(ServerEvent::RoundNagashiMangan {
                winners: vec![crate::protocol::NagashiManganWinner {
                    wind: Wind::West,
                    score_points: 8_000,
                }],
                scores: [21_000, 21_000, 37_000, 21_000],
                riichi_sticks: 0,
                player_hands: Vec::new(),
            }),
            ServerMessage::Resync {
                events: vec![
                    ServerEvent::OtherPlayerDrew {
                        player: Wind::South,
                        remaining_tiles: 60,
                    },
                    ServerEvent::TileDiscarded {
                        player: Wind::South,
                        tile: Tile::new(Tile::S9),
                        is_tsumogiri: true,
                        hand_index: None,
                    },
                ],
            },
            ServerMessage::GameOver {
                final_scores: [32000, 25000, 24000, 19000],
            },
            ServerMessage::PlayerConnectionChanged {
                seat: 2,
                connected: false,
            },
            ServerMessage::TurnTimer { seconds: 90 },
            ServerMessage::Error {
                code: ErrorCode::RoomNotFound,
                message: "no such room".to_string(),
            },
        ];

        for msg in messages {
            let decoded = roundtrip_server(msg.clone());
            assert_eq!(
                format!("{:?}", decoded),
                format!("{:?}", msg),
                "round-trip mismatch"
            );
        }
    }

    #[test]
    fn test_all_error_codes_roundtrip() {
        let codes = [
            ErrorCode::VersionMismatch,
            ErrorCode::RoomNotFound,
            ErrorCode::RoomFull,
            ErrorCode::NotHost,
            ErrorCode::NotInRoom,
            ErrorCode::GameInProgress,
            ErrorCode::InvalidAction,
            ErrorCode::BadMessage,
            ErrorCode::RateLimited,
        ];
        for code in codes {
            let msg = ServerMessage::Error {
                code,
                message: String::new(),
            };
            let decoded = roundtrip_server(msg);
            match decoded {
                ServerMessage::Error { code: c, .. } => assert_eq!(c, code),
                _ => panic!("variant changed"),
            }
        }
    }

    /// RoomState from an old server without `length` must fall back
    /// to East-only.
    #[test]
    fn test_room_state_without_length_defaults_to_east_only() {
        let json = r#"{"RoomState":{"code":"ABC234","seats":["Empty","Empty","Empty","Empty"],"host_seat":0,"your_seat":1}}"#;
        let decoded = ServerMessage::from_json(json).expect("decode");
        match decoded {
            ServerMessage::RoomState { length, .. } => assert_eq!(length, GameLength::EastOnly),
            _ => panic!("variant changed"),
        }
    }

    /// RoomState from an old server without `cpu_configs` must fall back
    /// to None (#245).
    #[test]
    fn test_room_state_without_cpu_configs_defaults_to_none() {
        let json = r#"{"RoomState":{"code":"ABC234","seats":["Empty","Empty","Empty","Empty"],"host_seat":0,"your_seat":1}}"#;
        let decoded = ServerMessage::from_json(json).expect("decode");
        match decoded {
            ServerMessage::RoomState { cpu_configs, .. } => assert_eq!(cpu_configs, None),
            _ => panic!("variant changed"),
        }
    }

    #[test]
    fn test_bad_json_is_rejected() {
        assert!(ClientMessage::from_json("not json").is_err());
        assert!(ServerMessage::from_json("{\"Unknown\":{}}").is_err());
    }

    /// RoundWon must round-trip through JSON with its structured yaku,
    /// dora, and rank intact (regression for #242, which switched from
    /// pre-formatted strings to enums for i18n).
    #[test]
    fn test_round_won_structured_roundtrip() {
        use mahjong_core::scoring::score::{DoraLabel, ScoreItem, ScoreRank};
        use mahjong_core::winning_hand::name::Kind;

        let msg = ServerMessage::Event(ServerEvent::RoundWon {
            winner: Wind::East,
            loser: Some(Wind::South),
            winning_tile: Tile::new(Tile::M1),
            scores: [35000, 15000, 25000, 25000],
            yaku_list: vec![
                (ScoreItem::Yaku(Kind::Riichi), 1),
                (ScoreItem::Yaku(Kind::AllInside), 1),
                (ScoreItem::Dora(DoraLabel::Dora), 2),
                (ScoreItem::Dora(DoraLabel::RedDora), 1),
            ],
            han: 5,
            fu: 40,
            score_points: 8000,
            rank: ScoreRank::Mangan,
            has_opened: true,
            uradora_indicators: vec![Tile::new(Tile::P3)],
            riichi_sticks: 1,
            honba: 2,
            honba_points: 600,
            player_hands: Vec::new(),
        });

        let decoded = roundtrip_server(msg.clone());
        assert_eq!(format!("{:?}", decoded), format!("{:?}", msg));
    }
}
