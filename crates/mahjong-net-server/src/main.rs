//! mahjong-net-server entry point.
//!
//! `PORT` picks the listen port (default 8080); logging is controlled by
//! `RUST_LOG` (e.g. `RUST_LOG=mahjong_net_server=debug`).
//!
//! Multi-machine setups also start an internal listener (default 8081,
//! `INTERNAL_PORT`) for cross-machine room lookups; on Fly.io it binds
//! only to the private-network (6PN) address.

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

    // The internal (cross-machine lookup) listener, never exposed
    // publicly: bound to the 6PN address (FLY_PRIVATE_IP) on Fly and to
    // loopback in local development.
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
        // A single machine works without the internal listener,
        // so keep starting.
        Err(e) => tracing::warn!("failed to bind internal listener {internal_addr}: {e}"),
    }

    // Enable ConnectInfo<SocketAddr> so the peer IP is available.
    axum::serve(
        listener,
        app_with_state(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server error");
}
