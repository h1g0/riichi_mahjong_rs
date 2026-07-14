//! Peer (same-app machines) discovery and lookup.
//!
//! In a multi-machine setup a room exists only in its creating machine's
//! memory. When a connection arrives with another machine's room code,
//! each peer's internal listener (`GET /internal/rooms/{code}`) is
//! queried for the owner's ID, and a `fly-replay` header makes the Fly
//! proxy forward the connection there.
//!
//! Discovery is chosen by environment variables:
//! - `MAHJONG_PEERS` - fixed comma-separated `host:port` list (local/tests)
//! - `FLY_APP_NAME` - Fly.io internal DNS (`<app>.internal` AAAA records)
//! - neither: no peers (single machine)

use std::future::Future;
use std::net::IpAddr;
use std::time::Duration;

use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Default internal-listener port.
pub const DEFAULT_INTERNAL_PORT: u16 = 8081;

/// Peer-query timeout (connect + response combined).
const QUERY_TIMEOUT: Duration = Duration::from_millis(500);

/// Maximum internal HTTP response size, including headers.
const MAX_RESPONSE_SIZE: usize = 8 * 1024;

/// Maximum redraws on peer collisions during room creation.
///
/// Bounds the loop even if a broken peer keeps claiming ownership;
/// past the cap, local uniqueness alone decides.
const CREATE_COLLISION_RETRIES: usize = 4;

/// How peers are discovered.
#[derive(Clone)]
enum PeerSource {
    /// No peers (single machine, local development)
    None,
    /// Fixed list (`MAHJONG_PEERS`)
    Static(Vec<String>),
    /// Fly.io internal DNS (`<app>.internal` AAAA records)
    FlyDns { app_name: String, port: u16 },
}

/// The peer set plus this machine's identity.
#[derive(Clone)]
pub struct Peers {
    /// This machine's ID (`FLY_MACHINE_ID`), used in `fly-replay: instance=`
    machine_id: Option<String>,
    /// This machine's 6PN address, used to exclude ourselves from DNS results
    private_ip: Option<IpAddr>,
    source: PeerSource,
}

impl Peers {
    /// Peer-less configuration (single machine; legacy tests).
    pub fn none() -> Self {
        Peers {
            machine_id: None,
            private_ip: None,
            source: PeerSource::None,
        }
    }

    /// Reads the configuration from environment variables.
    pub fn from_env() -> Self {
        let machine_id = std::env::var("FLY_MACHINE_ID")
            .ok()
            .filter(|s| !s.is_empty());
        let private_ip = std::env::var("FLY_PRIVATE_IP")
            .ok()
            .and_then(|s| s.parse().ok());
        let source = if let Ok(peers) = std::env::var("MAHJONG_PEERS") {
            PeerSource::Static(
                peers
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            )
        } else if let Ok(app_name) = std::env::var("FLY_APP_NAME") {
            PeerSource::FlyDns {
                app_name,
                port: internal_port_from_env(),
            }
        } else {
            PeerSource::None
        };
        Peers {
            machine_id,
            private_ip,
            source,
        }
    }

    /// Configuration with a fixed peer list (for tests).
    pub fn with_static(machine_id: Option<&str>, peers: Vec<String>) -> Self {
        Peers {
            machine_id: machine_id.map(String::from),
            private_ip: None,
            source: PeerSource::Static(peers),
        }
    }

    /// This machine's ID.
    pub fn machine_id(&self) -> Option<&str> {
        self.machine_id.as_deref()
    }

    /// Finds the peer owning the room code and returns its machine ID.
    ///
    /// Queries every peer concurrently and returns the first hit;
    /// failures (unreachable, timeout) count as "not owning".
    pub async fn find_room(&self, code: &str) -> Option<String> {
        let addrs = self.peer_addrs().await;
        if addrs.is_empty() {
            return None;
        }
        let queries = addrs.iter().map(|addr| query_peer(addr, code));
        first_some(queries).await
    }

    /// Generates a room code while checking peers for collisions.
    ///
    /// Up to the retry cap, each locally unused candidate from `generate`
    /// is checked against the peers. If every check collides, a fresh local
    /// candidate is returned without trusting further peer responses.
    pub async fn pick_unused_code(&self, mut generate: impl FnMut() -> String) -> String {
        for _ in 0..CREATE_COLLISION_RETRIES {
            let candidate = generate();
            if self.find_room(&candidate).await.is_none() {
                return candidate;
            }
            tracing::warn!(code = candidate, "room code collides with a peer; retrying");
        }
        // Room creation proceeds even when peer answers are
        // untrustworthy.
        tracing::warn!("giving up peer collision check; using locally unique code");
        generate()
    }

    /// The peer list as `host:port` entries.
    async fn peer_addrs(&self) -> Vec<String> {
        match &self.source {
            PeerSource::None => Vec::new(),
            PeerSource::Static(list) => list.clone(),
            PeerSource::FlyDns { app_name, port } => {
                let host = format!("{app_name}.internal:{port}");
                match tokio::net::lookup_host(&host).await {
                    Ok(addrs) => addrs
                        .filter(|addr| Some(addr.ip()) != self.private_ip)
                        .map(|addr| addr.to_string())
                        .collect(),
                    Err(e) => {
                        tracing::warn!("failed to resolve {host}: {e}");
                        Vec::new()
                    }
                }
            }
        }
    }
}

/// Resolves with the first successful query without waiting for slower peers.
async fn first_some<F, T>(futures: impl IntoIterator<Item = F>) -> Option<T>
where
    F: Future<Output = Option<T>>,
{
    let mut pending: FuturesUnordered<F> = futures.into_iter().collect();
    while let Some(result) = pending.next().await {
        if result.is_some() {
            return result;
        }
    }
    None
}

/// Internal-listener port from `INTERNAL_PORT`.
pub fn internal_port_from_env() -> u16 {
    std::env::var("INTERNAL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_INTERNAL_PORT)
}

/// Queries one peer for room ownership.
///
/// Sent as HTTP/1.0 so the response stays simple (Content-Length, no
/// chunking); a 200 yields the body (the owner's machine ID).
async fn query_peer(addr: &str, code: &str) -> Option<String> {
    let result = tokio::time::timeout(QUERY_TIMEOUT, async {
        let mut stream = TcpStream::connect(addr).await.ok()?;
        let request = format!("GET /internal/rooms/{code} HTTP/1.0\r\nHost: {addr}\r\n\r\n");
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut raw = Vec::new();
        stream
            .take((MAX_RESPONSE_SIZE + 1) as u64)
            .read_to_end(&mut raw)
            .await
            .ok()?;
        parse_response(&raw)
    })
    .await;
    result.ok().flatten()
}

/// Extracts the machine ID from the HTTP response; non-200 or
/// malformed yields None.
fn parse_response(raw: &[u8]) -> Option<String> {
    if raw.len() > MAX_RESPONSE_SIZE {
        return None;
    }
    let text = std::str::from_utf8(raw).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status_line = head.lines().next()?;
    if !status_line.starts_with("HTTP/1.") || status_line.split_whitespace().nth(1)? != "200" {
        return None;
    }
    let id = body.trim();
    // The ID lands verbatim in a fly-replay header, so validate its
    // format.
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::future::BoxFuture;

    #[test]
    fn test_parse_response_ok() {
        let raw = b"HTTP/1.0 200 OK\r\ncontent-length: 14\r\n\r\ne784079b449483";
        assert_eq!(parse_response(raw), Some("e784079b449483".to_string()));
    }

    #[test]
    fn test_parse_response_not_found() {
        let raw = b"HTTP/1.0 404 Not Found\r\ncontent-length: 0\r\n\r\n";
        assert_eq!(parse_response(raw), None);
    }

    #[test]
    fn test_parse_response_rejects_invalid_machine_id() {
        // Reject characters that could inject headers.
        let raw = b"HTTP/1.0 200 OK\r\n\r\nbad;id=x";
        assert_eq!(parse_response(raw), None);
        let empty = b"HTTP/1.0 200 OK\r\n\r\n";
        assert_eq!(parse_response(empty), None);
    }

    #[test]
    fn test_parse_response_rejects_garbage() {
        assert_eq!(parse_response(b"not http"), None);
        assert_eq!(parse_response(&[0xff, 0xfe]), None);
    }

    #[test]
    fn test_parse_response_rejects_oversized_body() {
        let mut raw = b"HTTP/1.0 200 OK\r\n\r\n".to_vec();
        raw.resize(MAX_RESPONSE_SIZE + 1, b'a');
        assert_eq!(parse_response(&raw), None);
    }

    #[tokio::test]
    async fn test_find_room_without_peers() {
        assert_eq!(Peers::none().find_room("ABCDEF").await, None);
    }

    #[tokio::test]
    async fn test_find_room_unreachable_peer_is_ignored() {
        // An unreachable peer counts as not owning.
        let peers = Peers::with_static(Some("self1"), vec!["127.0.0.1:1".to_string()]);
        assert_eq!(peers.find_room("ABCDEF").await, None);
    }

    #[tokio::test]
    async fn test_first_some_does_not_wait_for_pending_queries() {
        let never: BoxFuture<'static, Option<String>> = Box::pin(std::future::pending());
        let found: BoxFuture<'static, Option<String>> =
            Box::pin(async { Some("owner".to_string()) });

        let result = tokio::time::timeout(Duration::from_millis(100), first_some([never, found]))
            .await
            .expect("a successful peer should end the lookup immediately");

        assert_eq!(result.as_deref(), Some("owner"));
    }
}
