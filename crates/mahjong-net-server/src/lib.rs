//! Network server for online play.
//!
//! Serves WebSocket clients with a room-code lobby and server-
//! authoritative game flow; the game logic itself is delegated to
//! `mahjong_server::driver::GameDriver`.
//!
//! Layout:
//! - [`lobby`] - registry of room codes and room actors
//! - [`room`] - the room actor (one tokio task per room)
//! - [`connection`] - WebSocket handshake and message relay
//! - [`peers`] - peer discovery and room lookup across machines
//!
//! A room lives only in the memory of the machine that created it. In a
//! multi-machine setup, when a `/ws?room=CODE` connection lands on a
//! machine without that room, [`peers`] locates the owner and a
//! `fly-replay` header forwards the connection.

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

/// Application-wide shared state.
#[derive(Clone)]
pub struct AppState {
    /// Room registry
    pub lobby: Lobby,
    /// Per-IP join rate limiter
    pub rate_limiter: RateLimiter,
    /// Allowed browser Origins; `None` allows all
    pub allowed_origins: Option<Vec<String>>,
    /// Peer (other machine) discovery and lookup
    pub peers: Peers,
}

/// Builds the shared state from environment variables.
///
/// `ALLOWED_ORIGINS` accepts a comma-separated list of exact Origins.
/// The legacy `ALLOWED_ORIGIN` value is added to that list when set. If
/// neither contains an Origin, all are accepted. Peer configuration is
/// described at [`Peers::from_env`].
pub fn build_state(config: RoomConfig) -> AppState {
    let allowed_origins = std::env::var("ALLOWED_ORIGINS").ok();
    let allowed_origin = std::env::var("ALLOWED_ORIGIN").ok();
    AppState {
        lobby: Lobby::new(config),
        rate_limiter: RateLimiter::new(),
        allowed_origins: parse_allowed_origins(
            allowed_origins.as_deref(),
            allowed_origin.as_deref(),
        ),
        peers: Peers::from_env(),
    }
}

fn parse_allowed_origins(
    allowed_origins: Option<&str>,
    allowed_origin: Option<&str>,
) -> Option<Vec<String>> {
    let mut origins = Vec::new();
    for origin in allowed_origins
        .into_iter()
        .flat_map(|value| value.split(','))
        .chain(allowed_origin)
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
    {
        if !origins.iter().any(|existing| existing == origin) {
            origins.push(origin.to_owned());
        }
    }
    (!origins.is_empty()).then_some(origins)
}

/// Builds the public router: `/ws` for WebSocket, `/healthz` for
/// health checks.
pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/ws", get(connection::ws_handler))
        .with_state(state)
}

/// Builds the public router with state from the environment.
pub fn app(config: RoomConfig) -> Router {
    app_with_state(build_state(config))
}

/// Router for the internal (cross-machine lookup) API.
///
/// `GET /internal/rooms/{code}` returns 200 and this machine's ID when
/// it owns the room. Bind only to the private-network listener - never
/// expose it: the Fly proxy forwards every path to internal_port, so an
/// internal route on the public router would be reachable from outside.
pub fn internal_app(state: AppState) -> Router {
    Router::new()
        .route("/internal/rooms/{code}", get(internal_room_lookup))
        .with_state(state)
}

/// Answers a room-location lookup.
async fn internal_room_lookup(Path(code): Path<String>, State(state): State<AppState>) -> Response {
    // Without a machine ID there is no fly-replay target,
    // so owning the room still yields 404.
    match (state.peers.machine_id(), state.lobby.get(&code)) {
        (Some(id), Some(_)) => (StatusCode::OK, id.to_string()).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_allowed_origins;

    #[test]
    fn test_parse_allowed_origins_combines_current_and_legacy_settings() {
        assert_eq!(
            parse_allowed_origins(
                Some("https://web.example.com, https://html.example.com"),
                Some("https://legacy.example.com"),
            ),
            Some(vec![
                "https://web.example.com".to_string(),
                "https://html.example.com".to_string(),
                "https://legacy.example.com".to_string(),
            ])
        );
    }

    #[test]
    fn test_parse_allowed_origins_ignores_empty_and_duplicate_values() {
        assert_eq!(
            parse_allowed_origins(
                Some(" ,https://web.example.com,https://web.example.com, "),
                Some(""),
            ),
            Some(vec!["https://web.example.com".to_string()])
        );
        assert_eq!(parse_allowed_origins(Some(" , "), None), None);
        assert_eq!(parse_allowed_origins(None, None), None);
    }
}
