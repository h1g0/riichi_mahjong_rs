//! WebSocketトランスポート
//!
//! macroquad のフレームループから毎フレーム poll できる、
//! ノンブロッキングな WebSocket 抽象。
//!
//! - ネイティブ: tungstenite を別スレッドで動かし、mpsc チャネルで橋渡しする
//! - WASM: フェーズ4（手書きJSグルー）まではスタブ（常にエラーを返す）

/// トランスポートで発生したイベント
// WASMスタブは Error しか生成しないが、ネイティブでは全バリアントを使う
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Debug, Clone)]
pub enum WsEvent {
    /// 接続が確立した
    Opened,
    /// テキストメッセージを受信した
    Message(String),
    /// 接続が閉じられた
    Closed,
    /// エラーが発生した（接続失敗・通信エラー）
    Error(String),
}

/// ノンブロッキングな WebSocket 接続
///
/// `RemoteAdapter` はこのトレイト経由で通信するため、
/// テストではスクリプト化したモックに差し替えられる。
pub trait Transport {
    /// テキストフレームを送信する（未接続・切断後の送信は無視される）
    fn send_text(&mut self, text: &str);

    /// 発生したイベントをすべて取り出す（ブロックしない）
    fn poll(&mut self) -> Vec<WsEvent>;
}

/// 既定の接続先URLを返す
pub fn default_server_url() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("MAHJONG_SERVER_URL").unwrap_or_else(|_| "ws://127.0.0.1:8080/ws".to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        // フェーズ4で window.MAHJONG_SERVER_URL から取得する
        "ws://127.0.0.1:8080/ws".to_string()
    }
}

/// サーバへの接続を開始し、トランスポートを返す
///
/// 接続は非同期に進行し、結果は `poll()` の `Opened` / `Error` で通知される。
pub fn connect(url: &str) -> Box<dyn Transport> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Box::new(native::NativeTransport::connect(url))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = url;
        Box::new(wasm_stub::StubTransport::new())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::net::TcpStream;
    use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
    use std::time::Duration;

    use tungstenite::Message;
    use tungstenite::stream::MaybeTlsStream;

    use super::{Transport, WsEvent};

    /// 受信待ちのポーリング間隔
    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    /// ネイティブ用トランスポート: 通信スレッドとチャネルで橋渡しする
    pub struct NativeTransport {
        out_tx: Sender<String>,
        in_rx: Receiver<WsEvent>,
    }

    impl NativeTransport {
        pub fn connect(url: &str) -> Self {
            let (out_tx, out_rx) = channel::<String>();
            let (in_tx, in_rx) = channel::<WsEvent>();
            let url = url.to_string();
            std::thread::spawn(move || run_socket(&url, &out_rx, &in_tx));
            NativeTransport { out_tx, in_rx }
        }
    }

    impl Transport for NativeTransport {
        fn send_text(&mut self, text: &str) {
            // スレッド終了後（切断後）の送信は無視する
            let _ = self.out_tx.send(text.to_string());
        }

        fn poll(&mut self) -> Vec<WsEvent> {
            let mut events = Vec::new();
            // Err はキューが空か、スレッド終了済み（Closed/Error は通知済み）
            while let Ok(event) = self.in_rx.try_recv() {
                events.push(event);
            }
            events
        }
    }

    /// 通信スレッド本体
    fn run_socket(url: &str, out_rx: &Receiver<String>, in_tx: &Sender<WsEvent>) {
        let (mut socket, _) = match tungstenite::connect(url) {
            Ok(ok) => ok,
            Err(e) => {
                let _ = in_tx.send(WsEvent::Error(format!("接続に失敗しました: {e}")));
                return;
            }
        };

        // 送受信を1ループで回すためノンブロッキングにする
        if let Err(e) = set_nonblocking(socket.get_mut()) {
            let _ = in_tx.send(WsEvent::Error(format!("ソケット設定に失敗しました: {e}")));
            return;
        }

        if in_tx.send(WsEvent::Opened).is_err() {
            return;
        }

        loop {
            // 送信キューを書き込みバッファへ移す
            loop {
                match out_rx.try_recv() {
                    Ok(text) => {
                        if let Err(e) = socket.write(Message::text(text)) {
                            let _ = in_tx.send(WsEvent::Error(format!("送信エラー: {e}")));
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // トランスポートが破棄された: 接続を閉じて終了
                        let _ = socket.close(None);
                        let _ = socket.flush();
                        return;
                    }
                }
            }

            // 書き込みバッファを送出する（Pong の自動応答もここで送られる）
            match socket.flush() {
                Ok(()) => {}
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    let _ = in_tx.send(WsEvent::Closed);
                    return;
                }
                Err(e) => {
                    let _ = in_tx.send(WsEvent::Error(format!("送信エラー: {e}")));
                    return;
                }
            }

            // 受信
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if in_tx.send(WsEvent::Message(text.to_string())).is_err() {
                        let _ = socket.close(None);
                        return;
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = in_tx.send(WsEvent::Closed);
                    return;
                }
                // Ping/Pong は下層が応答をキューイングする。Binary は使わない
                Ok(_) => {}
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    let _ = in_tx.send(WsEvent::Closed);
                    return;
                }
                Err(e) => {
                    let _ = in_tx.send(WsEvent::Error(format!("通信エラー: {e}")));
                    return;
                }
            }
        }
    }

    /// 下層の TCP ストリームをノンブロッキングに設定する
    fn set_nonblocking(stream: &mut MaybeTlsStream<TcpStream>) -> std::io::Result<()> {
        match stream {
            MaybeTlsStream::Plain(s) => s.set_nonblocking(true),
            MaybeTlsStream::Rustls(s) => s.get_ref().set_nonblocking(true),
            _ => Ok(()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_stub {
    use super::{Transport, WsEvent};

    /// WASM用スタブ: 接続せず、一度だけエラーを通知する
    pub struct StubTransport {
        reported: bool,
    }

    impl StubTransport {
        pub fn new() -> Self {
            StubTransport { reported: false }
        }
    }

    impl Transport for StubTransport {
        fn send_text(&mut self, _text: &str) {}

        fn poll(&mut self) -> Vec<WsEvent> {
            if self.reported {
                Vec::new()
            } else {
                self.reported = true;
                vec![WsEvent::Error(
                    "このビルドではオンライン対戦は未対応です".to_string(),
                )]
            }
        }
    }
}
