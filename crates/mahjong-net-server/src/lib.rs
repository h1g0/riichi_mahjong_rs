//! オンライン対戦用ネットワークサーバ
//!
//! WebSocket でクライアントと接続し、ルームコード制のロビーと
//! サーバ権威のゲーム進行を提供する。ゲームロジック自体は
//! `mahjong_server::driver::GameDriver` に委譲する。
//!
//! 構成:
//! - [`lobby`] — ルームコードとルームアクターのレジストリ
//! - [`room`] — ルームアクター（1ルーム = 1 tokio タスク）
//! - [`connection`] — WebSocket 接続のハンドシェイクとメッセージ中継

pub mod connection;
pub mod lobby;
pub mod room;

use axum::Router;
use axum::routing::get;

use lobby::Lobby;
use room::RoomConfig;

/// アプリケーション全体の共有状態
#[derive(Clone)]
pub struct AppState {
    /// ルームレジストリ
    pub lobby: Lobby,
}

/// ルーターを構築する
///
/// `/ws` が WebSocket エンドポイント、`/healthz` がヘルスチェック。
pub fn app(config: RoomConfig) -> Router {
    let state = AppState {
        lobby: Lobby::new(config),
    };
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/ws", get(connection::ws_handler))
        .with_state(state)
}
