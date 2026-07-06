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
//! - [`peers`] — 複数マシン構成でのピア発見・ルーム所在の照会
//!
//! ルームは生成されたマシンのメモリ上にのみ存在する。複数マシン構成では
//! `/ws?room=CODE` のコードを持たないマシンに接続が着地したとき、
//! [`peers`] で所持マシンを特定し `fly-replay` ヘッダで転送させる。

pub mod connection;
pub mod lobby;
pub mod peers;
pub mod ratelimit;
pub mod room;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use lobby::Lobby;
use peers::Peers;
use ratelimit::RateLimiter;
use room::RoomConfig;

/// アプリケーション全体の共有状態
#[derive(Clone)]
pub struct AppState {
    /// ルームレジストリ
    pub lobby: Lobby,
    /// IPごとの入室レート制限
    pub rate_limiter: RateLimiter,
    /// 接続を許可する Origin（None なら全許可）
    pub allowed_origin: Option<String>,
    /// ピア（他マシン）の発見・照会
    pub peers: Peers,
}

/// 環境変数から共有状態を構築する
///
/// `ALLOWED_ORIGIN` が設定されていれば、その Origin からの
/// WebSocket 接続のみを許可する（未設定なら全許可）。
/// ピア構成は [`Peers::from_env`] を参照。
pub fn build_state(config: RoomConfig) -> AppState {
    let allowed_origin = std::env::var("ALLOWED_ORIGIN")
        .ok()
        .filter(|s| !s.is_empty());
    AppState {
        lobby: Lobby::new(config),
        rate_limiter: RateLimiter::new(),
        allowed_origin,
        peers: Peers::from_env(),
    }
}

/// 公開ルーターを構築する
///
/// `/ws` が WebSocket エンドポイント、`/healthz` がヘルスチェック。
pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/ws", get(connection::ws_handler))
        .with_state(state)
}

/// 環境変数から状態を構築して公開ルーターを作る
pub fn app(config: RoomConfig) -> Router {
    app_with_state(build_state(config))
}

/// 内部（マシン間照会）API のルーター
///
/// `GET /internal/rooms/{code}` — ルームを所持していれば 200 と自マシン ID を返す。
/// プライベートネットワーク専用のリスナーにのみ bind し、公開してはならない
/// （Fly Proxy は全パスを internal_port へ転送するため、公開ルーターに
/// 内部ルートを生やすと外部から到達できてしまう）。
pub fn internal_app(state: AppState) -> Router {
    Router::new()
        .route("/internal/rooms/{code}", get(internal_room_lookup))
        .with_state(state)
}

/// ルーム所在の照会に応答する
async fn internal_room_lookup(Path(code): Path<String>, State(state): State<AppState>) -> Response {
    // マシン ID が無いと fly-replay の宛先にできないため、所持していても 404
    match (state.peers.machine_id(), state.lobby.get(&code)) {
        (Some(id), Some(_)) => (StatusCode::OK, id.to_string()).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
