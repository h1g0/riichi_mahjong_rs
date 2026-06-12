//! オンライン対戦用のネットワークメッセージ定義
//!
//! WebSocket のテキストフレームで JSON としてやり取りするエンベロープ型。
//! ゲーム内のやり取りは既存の `ClientAction` / `ServerEvent` をそのまま包む。

use serde::{Deserialize, Serialize};

use super::{ClientAction, ServerEvent};

/// プロトコルバージョン
///
/// 互換性のない変更を入れる際にインクリメントする。
/// `Hello` で照合し、不一致なら `ErrorCode::VersionMismatch` で切断する。
pub const PROTOCOL_VERSION: u32 = 1;

/// クライアントからサーバへのメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// 接続時のハンドシェイク（最初の1通目でなければならない）
    Hello {
        /// クライアントのプロトコルバージョン
        protocol_version: u32,
        /// 再接続時に提示するセッショントークン
        session_token: Option<String>,
        /// 表示名
        display_name: String,
    },

    /// ルームを作成する（作成者がホスト）
    CreateRoom {
        /// 東風戦(1)か東南戦(2)か
        round_count: u8,
    },

    /// ルームコードを指定して参加する
    JoinRoom {
        /// 6文字のルームコード
        code: String,
    },

    /// ルームから退出する
    LeaveRoom,

    /// 対局を開始する（ホストのみ。空席はCPUで埋める）
    StartGame,

    /// ゲーム内アクション
    Action(ClientAction),

    /// 局結果画面を確認し、次の局へ進む準備ができた
    ReadyNextRound,
}

/// サーバからクライアントへのメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// ハンドシェイク応答
    Welcome {
        /// 再接続に使うセッショントークン
        session_token: String,
        /// サーバのプロトコルバージョン
        protocol_version: u32,
    },

    /// ルームの状態（参加・退出・接続状態の変化時に全員へ送信）
    RoomState {
        /// ルームコード
        code: String,
        /// 各座席の状態（座席インデックス順）
        seats: [SeatInfo; 4],
        /// ホストの座席インデックス
        host_seat: usize,
        /// 受信者自身の座席インデックス
        your_seat: usize,
    },

    /// ゲーム内イベント
    Event(ServerEvent),

    /// 再接続時の状態再同期（現在の局の開始からのイベント再生）
    Resync {
        /// 現在の局の `GameStarted` 以降のイベント列
        events: Vec<ServerEvent>,
    },

    /// ゲーム終了
    GameOver {
        /// 最終得点
        final_scores: [i32; 4],
    },

    /// プレイヤーの接続状態が変化した
    PlayerConnectionChanged {
        /// 座席インデックス
        seat: usize,
        /// 接続中か
        connected: bool,
    },

    /// エラー通知
    Error {
        /// エラーコード
        code: ErrorCode,
        /// 補足メッセージ（デバッグ用。表示文言はクライアント側で組み立てる）
        message: String,
    },
}

/// 座席の状態
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeatInfo {
    /// 空席
    Empty,
    /// CPU
    Cpu,
    /// 人間プレイヤー
    Human {
        /// 表示名
        name: String,
        /// 接続中か
        connected: bool,
    },
}

/// エラーコード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// プロトコルバージョン不一致
    VersionMismatch,
    /// ルームが存在しない
    RoomNotFound,
    /// ルームが満席
    RoomFull,
    /// ホスト専用の操作
    NotHost,
    /// ルームに参加していない
    NotInRoom,
    /// 対局中のため実行できない
    GameInProgress,
    /// 無効なアクション（手番違い・フェーズ違いなど）
    InvalidAction,
    /// メッセージを解釈できない
    BadMessage,
    /// レート制限超過
    RateLimited,
}

impl ClientMessage {
    /// JSON 文字列にエンコードする
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// JSON 文字列からデコードする
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl ServerMessage {
    /// JSON 文字列にエンコードする
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// JSON 文字列からデコードする
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
            ClientMessage::CreateRoom { round_count: 2 },
            ClientMessage::JoinRoom {
                code: "ABC234".to_string(),
            },
            ClientMessage::LeaveRoom,
            ClientMessage::StartGame,
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
                    SeatInfo::Cpu,
                    SeatInfo::Empty,
                ],
                host_seat: 0,
                your_seat: 1,
            },
            ServerMessage::Event(ServerEvent::TileDrawn {
                tile: Tile::new(Tile::P5),
                remaining_tiles: 69,
                can_tsumo: false,
                can_riichi: true,
                is_furiten: false,
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

    #[test]
    fn test_bad_json_is_rejected() {
        assert!(ClientMessage::from_json("not json").is_err());
        assert!(ServerMessage::from_json("{\"Unknown\":{}}").is_err());
    }
}
