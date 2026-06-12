//! mahjong-net-server エントリポイント
//!
//! `PORT` 環境変数でリッスンポートを指定する（デフォルト 8080）。
//! ログは `RUST_LOG` で制御する（例: `RUST_LOG=mahjong_net_server=debug`）。

use std::net::SocketAddr;

use mahjong_net_server::app;
use mahjong_net_server::room::RoomConfig;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mahjong_net_server=info".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    tracing::info!("listening on {addr}");

    axum::serve(listener, app(RoomConfig::default()))
        .await
        .expect("server error");
}
