//! WebSocketトランスポート
//!
//! macroquad のフレームループから毎フレーム poll できる、
//! ノンブロッキングな WebSocket 抽象。
//!
//! - ネイティブ: tungstenite を別スレッドで動かし、mpsc チャネルで橋渡しする
//! - WASM: 手書きJSグルー (crates/mahjong-client/js/ws.js) の関数を
//!   extern "C" で呼ぶ（wasm-bindgen 不使用。wasm_rng.rs と同じ方針）

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
        // index.html で window.MAHJONG_SERVER_URL が設定されていればそれを使う
        wasm::page_server_url().unwrap_or_else(|| "ws://127.0.0.1:8080/ws".to_string())
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
        Box::new(wasm::WasmTransport::connect(url))
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
            ensure_crypto_provider();
            let (out_tx, out_rx) = channel::<String>();
            let (in_tx, in_rx) = channel::<WsEvent>();
            let url = url.to_string();
            std::thread::spawn(move || run_socket(&url, &out_rx, &in_tx));
            NativeTransport { out_tx, in_rx }
        }
    }

    /// rustls のプロセス既定 CryptoProvider を一度だけ用意する
    ///
    /// rustls 0.23 は wss 接続前にプロセス既定のプロバイダが必要で、
    /// 未設定だと tungstenite が接続時に panic する。複数の機能で rustls を
    /// 引き込んでも安全なよう、`install_default` の失敗（設定済み）は無視する。
    fn ensure_crypto_provider() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
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
mod wasm {
    use super::{Transport, WsEvent};

    // 接続ステータス（ws.js と一致させる）
    const STATUS_OPEN: i32 = 1;
    const STATUS_CLOSED: i32 = 2;
    const STATUS_ERROR: i32 = 3;

    // ws.js が miniquad のプラグイン機構で importObject.env に注入する関数群
    unsafe extern "C" {
        fn mahjong_ws_connect(url_ptr: *const u8, url_len: usize) -> i32;
        fn mahjong_ws_status(handle: i32) -> i32;
        fn mahjong_ws_send(handle: i32, ptr: *const u8, len: usize) -> i32;
        fn mahjong_ws_next_msg_len(handle: i32) -> i32;
        fn mahjong_ws_read_msg(handle: i32, buf_ptr: *mut u8);
        fn mahjong_ws_close(handle: i32);
        fn mahjong_ws_default_url(buf_ptr: *mut u8, cap: usize) -> i32;
    }

    /// ws.js プラグインのバージョン照合用
    ///
    /// mq_js_bundle.js の init_plugins が `{プラグイン名}_crate_version` を
    /// 呼び、JS側のバージョンと一致するか検証する。
    #[unsafe(no_mangle)]
    pub extern "C" fn mahjong_ws_crate_version() -> u32 {
        1
    }

    /// ページに設定された接続先URL (window.MAHJONG_SERVER_URL) を取得する
    pub fn page_server_url() -> Option<String> {
        let mut buf = vec![0u8; 1024];
        let len = unsafe { mahjong_ws_default_url(buf.as_mut_ptr(), buf.len()) };
        if len <= 0 {
            return None;
        }
        buf.truncate(len as usize);
        String::from_utf8(buf).ok()
    }

    /// WASM用トランスポート: ws.js の WebSocket をハンドル経由で操作する
    pub struct WasmTransport {
        handle: i32,
        /// Opened を通知済みか
        opened_reported: bool,
        /// Closed/Error を通知済みか（以後 poll は空を返す）
        terminated: bool,
    }

    impl WasmTransport {
        pub fn connect(url: &str) -> Self {
            let handle = unsafe { mahjong_ws_connect(url.as_ptr(), url.len()) };
            WasmTransport {
                handle,
                opened_reported: false,
                terminated: false,
            }
        }
    }

    impl Transport for WasmTransport {
        fn send_text(&mut self, text: &str) {
            // 失敗（未接続・切断後）はステータス変化として poll で検出される
            unsafe {
                mahjong_ws_send(self.handle, text.as_ptr(), text.len());
            }
        }

        fn poll(&mut self) -> Vec<WsEvent> {
            let mut events = Vec::new();
            if self.terminated {
                return events;
            }

            let status = unsafe { mahjong_ws_status(self.handle) };
            if status == STATUS_OPEN && !self.opened_reported {
                self.opened_reported = true;
                events.push(WsEvent::Opened);
            }

            // 受信済みメッセージを取り出す（長さ取得 → コピーの2段階）
            loop {
                let len = unsafe { mahjong_ws_next_msg_len(self.handle) };
                if len < 0 {
                    break;
                }
                let mut buf = vec![0u8; len as usize];
                unsafe {
                    mahjong_ws_read_msg(self.handle, buf.as_mut_ptr());
                }
                match String::from_utf8(buf) {
                    Ok(text) => events.push(WsEvent::Message(text)),
                    Err(_) => events.push(WsEvent::Error(
                        "サーバからのメッセージを解釈できません".to_string(),
                    )),
                }
            }

            // 終了状態は受信済みメッセージを流し切ってから通知する
            match status {
                STATUS_CLOSED => {
                    self.terminated = true;
                    events.push(WsEvent::Closed);
                }
                STATUS_ERROR => {
                    self.terminated = true;
                    events.push(WsEvent::Error("WebSocket接続エラー".to_string()));
                }
                _ => {}
            }

            events
        }
    }

    impl Drop for WasmTransport {
        fn drop(&mut self) {
            unsafe {
                mahjong_ws_close(self.handle);
            }
        }
    }
}
