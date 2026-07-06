//! ピア（同一アプリの他マシン）の発見と照会
//!
//! 複数マシン構成では、ルームは生成されたマシンのメモリ上にのみ存在する。
//! 他マシンが持つルームコードで接続要求を受けたときは、各ピアの内部リスナー
//! （`GET /internal/rooms/{code}`）へ照会して所持マシンの ID を特定し、
//! `fly-replay` ヘッダで Fly Proxy にそのマシンへ転送させる。
//!
//! ピアの発見方法は環境変数で決まる:
//! - `MAHJONG_PEERS` — カンマ区切りの `host:port` 固定リスト（ローカル・テスト用）
//! - `FLY_APP_NAME` — Fly.io 内部 DNS（`<app>.internal` の AAAA レコード）
//! - どちらも無ければピアなし（単一マシン運用）

use std::net::IpAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// 内部リスナーのデフォルトポート
pub const DEFAULT_INTERNAL_PORT: u16 = 8081;

/// ピア照会のタイムアウト（接続 + 応答の合計）
const QUERY_TIMEOUT: Duration = Duration::from_millis(500);

/// ルーム作成時にピア衝突で引き直す最大回数
///
/// ピアが誤って常に「所持している」と答え続けても、ルーム作成が
/// 無限ループしないための上限。上限に達したらローカルの一意性のみで確定する。
const CREATE_COLLISION_RETRIES: usize = 4;

/// ピアの発見方法
#[derive(Clone)]
enum PeerSource {
    /// ピアなし（単一マシン・ローカル開発）
    None,
    /// 固定リスト（`MAHJONG_PEERS`）
    Static(Vec<String>),
    /// Fly.io 内部 DNS（`<app>.internal` の AAAA レコード）
    FlyDns { app_name: String, port: u16 },
}

/// ピア一覧と自マシンの識別情報
#[derive(Clone)]
pub struct Peers {
    /// 自マシンの ID（`FLY_MACHINE_ID`）。`fly-replay: instance=` に使う
    machine_id: Option<String>,
    /// 自マシンの 6PN アドレス（DNS 結果から自分を除外するのに使う）
    private_ip: Option<IpAddr>,
    source: PeerSource,
}

impl Peers {
    /// ピアなしの構成（単一マシン運用・既存テスト用）
    pub fn none() -> Self {
        Peers {
            machine_id: None,
            private_ip: None,
            source: PeerSource::None,
        }
    }

    /// 環境変数から構成を読み取る
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

    /// 固定ピアリストで構成する（テスト用）
    pub fn with_static(machine_id: Option<&str>, peers: Vec<String>) -> Self {
        Peers {
            machine_id: machine_id.map(String::from),
            private_ip: None,
            source: PeerSource::Static(peers),
        }
    }

    /// 自マシンの ID
    pub fn machine_id(&self) -> Option<&str> {
        self.machine_id.as_deref()
    }

    /// ルームコードを所持しているピアを探し、そのマシン ID を返す
    ///
    /// 全ピアへ同時に照会し、最初に見つかったものを返す。
    /// 照会失敗（接続不可・タイムアウト）は「所持していない」として扱う。
    pub async fn find_room(&self, code: &str) -> Option<String> {
        let addrs = self.peer_addrs().await;
        if addrs.is_empty() {
            return None;
        }
        let queries = addrs.iter().map(|addr| query_peer(addr, code));
        futures_util::future::join_all(queries)
            .await
            .into_iter()
            .flatten()
            .next()
    }

    /// ルーム作成の候補コードが他マシンと衝突しないか確認しつつ生成する
    ///
    /// `generate` が返す候補（ローカルでは未使用のもの）をピアに照会し、
    /// 衝突しないコードを返す。照会の上限回数を超えたら最後の候補で確定する。
    pub async fn pick_unused_code(&self, mut generate: impl FnMut() -> String) -> String {
        for _ in 0..CREATE_COLLISION_RETRIES {
            let candidate = generate();
            if self.find_room(&candidate).await.is_none() {
                return candidate;
            }
            tracing::warn!(code = candidate, "room code collides with a peer; retrying");
        }
        // ピアの応答が信用できない場合でもルーム作成は継続する
        tracing::warn!("giving up peer collision check; using locally unique code");
        generate()
    }

    /// ピア一覧（`host:port`）を得る
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

/// 内部リスナーのポートを環境変数 `INTERNAL_PORT` から読む
pub fn internal_port_from_env() -> u16 {
    std::env::var("INTERNAL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_INTERNAL_PORT)
}

/// 1つのピアへルーム所持を照会する
///
/// 応答が単純（Content-Length 付き・非チャンク）になるよう HTTP/1.0 で
/// リクエストし、200 ならボディ（所持マシンの ID）を返す。
async fn query_peer(addr: &str, code: &str) -> Option<String> {
    let result = tokio::time::timeout(QUERY_TIMEOUT, async {
        let mut stream = TcpStream::connect(addr).await.ok()?;
        let request = format!("GET /internal/rooms/{code} HTTP/1.0\r\nHost: {addr}\r\n\r\n");
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.ok()?;
        parse_response(&raw)
    })
    .await;
    result.ok().flatten()
}

/// HTTP 応答からマシン ID を取り出す（200 以外・不正な形式は None）
fn parse_response(raw: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(raw).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status_line = head.lines().next()?;
    if !status_line.starts_with("HTTP/1.") || status_line.split_whitespace().nth(1)? != "200" {
        return None;
    }
    let id = body.trim();
    // fly-replay ヘッダにそのまま埋め込むため、マシン ID の形式を検証する
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // ヘッダインジェクションになりうる文字は拒否する
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

    #[tokio::test]
    async fn test_find_room_without_peers() {
        assert_eq!(Peers::none().find_room("ABCDEF").await, None);
    }

    #[tokio::test]
    async fn test_find_room_unreachable_peer_is_ignored() {
        // 接続できないピアは「所持していない」として扱う
        let peers = Peers::with_static(Some("self1"), vec!["127.0.0.1:1".to_string()]);
        assert_eq!(peers.find_room("ABCDEF").await, None);
    }
}
