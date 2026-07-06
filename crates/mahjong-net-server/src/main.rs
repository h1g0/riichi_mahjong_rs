//! mahjong-net-server エントリポイント
//!
//! `PORT` 環境変数でリッスンポートを指定する（デフォルト 8080）。
//! ログは `RUST_LOG` で制御する（例: `RUST_LOG=mahjong_net_server=debug`）。
//!
//! 複数マシン構成ではマシン間のルーム所在照会のため、公開リスナーとは別に
//! 内部リスナー（デフォルト 8081、`INTERNAL_PORT` で変更可）を起動する。
//! Fly.io 上ではプライベートネットワーク（6PN）のアドレスにのみ bind する。

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use mahjong_net_server::room::RoomConfig;
use mahjong_net_server::{app_with_state, build_state, internal_app, peers};

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

    let state = build_state(RoomConfig::default());

    // 内部（マシン間照会）リスナー。公開サービスには含めない。
    // Fly 上では 6PN アドレス（FLY_PRIVATE_IP）にのみ bind し、
    // ローカル開発ではループバックに bind する。
    let internal_host: IpAddr = std::env::var("FLY_PRIVATE_IP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let internal_addr = SocketAddr::from((internal_host, peers::internal_port_from_env()));
    match tokio::net::TcpListener::bind(internal_addr).await {
        Ok(internal_listener) => {
            tracing::info!("internal listener on {internal_addr}");
            let internal_state = state.clone();
            tokio::spawn(async move {
                if let Err(e) = axum::serve(internal_listener, internal_app(internal_state)).await {
                    tracing::error!("internal listener error: {e}");
                }
            });
        }
        // 内部リスナーが無くても単一マシンとしては動けるため、起動は続ける
        Err(e) => tracing::warn!("failed to bind internal listener {internal_addr}: {e}"),
    }

    // ConnectInfo<SocketAddr> を有効にして接続元IPを取得できるようにする
    axum::serve(
        listener,
        app_with_state(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server error");
}
