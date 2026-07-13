//! WebSocket transport.
//!
//! A non-blocking WebSocket abstraction pollable every frame from
//! macroquad's loop.
//!
//! - Native: tungstenite on a worker thread bridged over mpsc channels.
//! - WASM: extern "C" calls into hand-written JS glue
//!   (crates/mahjong-client/js/ws.js); no wasm-bindgen, same policy as
//!   wasm_rng.rs.

/// An event produced by the transport.
// The WASM stub only produces Error; native uses every variant.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Debug, Clone)]
pub enum WsEvent {
    /// The connection opened
    Opened,
    /// A text message arrived
    Message(String),
    /// The connection closed
    Closed,
    /// A connection or transport error.
    ///
    /// The string is a technical detail for logs; user-facing text is
    /// localized and built client-side.
    Error(String),
}

/// A non-blocking WebSocket connection.
///
/// `RemoteAdapter` talks through this trait, so tests can substitute a
/// scripted mock.
pub trait Transport {
    /// Sends a text frame; ignored before opening or after closing.
    fn send_text(&mut self, text: &str);

    /// Drains pending events without blocking.
    fn poll(&mut self) -> Vec<WsEvent>;
}

/// The default server URL.
pub fn default_server_url() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("MAHJONG_SERVER_URL").unwrap_or_else(|_| "ws://127.0.0.1:8080/ws".to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Prefer window.MAHJONG_SERVER_URL when index.html sets it.
        wasm::page_server_url().unwrap_or_else(|| "ws://127.0.0.1:8080/ws".to_string())
    }
}

/// Appends the room code as a `room=CODE` query parameter.
///
/// A multi-machine server uses it to locate the room's owning machine
/// and forward the connection before the WebSocket upgrade. The code is
/// user input, so URL-unsafe characters are escaped.
pub fn url_with_room(url: &str, code: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    let encoded: String = code
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("{url}{separator}room={encoded}")
}

/// Starts connecting to the server and returns the transport.
///
/// Connecting proceeds asynchronously; the outcome arrives via `poll()`
/// as `Opened` or `Error`.
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

    /// Polling interval while waiting for messages.
    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    /// Native transport bridging a worker thread over channels.
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

    /// Installs rustls's process-default CryptoProvider exactly once.
    ///
    /// rustls 0.23 requires a process default before a wss connection,
    /// or tungstenite panics on connect. `install_default` failing
    /// (already set) is ignored so multiple features can pull in rustls
    /// safely.
    fn ensure_crypto_provider() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    impl Transport for NativeTransport {
        fn send_text(&mut self, text: &str) {
            // Sends after the thread exits (disconnected) are ignored.
            let _ = self.out_tx.send(text.to_string());
        }

        fn poll(&mut self) -> Vec<WsEvent> {
            let mut events = Vec::new();
            // Err means an empty queue or a finished thread
            // (Closed/Error already delivered).
            while let Ok(event) = self.in_rx.try_recv() {
                events.push(event);
            }
            events
        }
    }

    /// The worker thread body.
    fn run_socket(url: &str, out_rx: &Receiver<String>, in_tx: &Sender<WsEvent>) {
        let (mut socket, _) = match tungstenite::connect(url) {
            Ok(ok) => ok,
            Err(e) => {
                let _ = in_tx.send(WsEvent::Error(format!("connect failed: {e}")));
                return;
            }
        };

        // Non-blocking so one loop serves both directions.
        if let Err(e) = set_nonblocking(socket.get_mut()) {
            let _ = in_tx.send(WsEvent::Error(format!("socket setup failed: {e}")));
            return;
        }

        if in_tx.send(WsEvent::Opened).is_err() {
            return;
        }

        loop {
            loop {
                match out_rx.try_recv() {
                    Ok(text) => {
                        match socket.write(Message::text(text)) {
                            Ok(()) => {}
                            // Even on WouldBlock the frame is queued in
                            // the internal buffer; a later flush retries.
                            Err(tungstenite::Error::Io(ref e))
                                if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(e) => {
                                let _ = in_tx.send(WsEvent::Error(format!("send failed: {e}")));
                                return;
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // The transport was dropped: close and exit.
                        let _ = socket.close(None);
                        let _ = socket.flush();
                        return;
                    }
                }
            }

            // Flush the write buffer; automatic Pong replies go
            // out here too.
            match socket.flush() {
                Ok(()) => {}
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    let _ = in_tx.send(WsEvent::Closed);
                    return;
                }
                Err(e) => {
                    let _ = in_tx.send(WsEvent::Error(format!("flush failed: {e}")));
                    return;
                }
            }

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
                // The layer below queues Ping/Pong replies;
                // Binary is unused.
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
                    let _ = in_tx.send(WsEvent::Error(format!("receive failed: {e}")));
                    return;
                }
            }
        }
    }

    /// Makes the underlying TCP stream non-blocking.
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

    // Connection status codes, matching ws.js.
    const STATUS_OPEN: i32 = 1;
    const STATUS_CLOSED: i32 = 2;
    const STATUS_ERROR: i32 = 3;

    // Functions ws.js injects into importObject.env via miniquad's
    // plugin mechanism.
    unsafe extern "C" {
        fn mahjong_ws_connect(url_ptr: *const u8, url_len: usize) -> i32;
        fn mahjong_ws_status(handle: i32) -> i32;
        fn mahjong_ws_send(handle: i32, ptr: *const u8, len: usize) -> i32;
        fn mahjong_ws_next_msg_len(handle: i32) -> i32;
        fn mahjong_ws_read_msg(handle: i32, buf_ptr: *mut u8);
        fn mahjong_ws_close(handle: i32);
        fn mahjong_ws_default_url(buf_ptr: *mut u8, cap: usize) -> i32;
    }

    /// Version handshake for the ws.js plugin: mq_js_bundle.js's
    /// init_plugins calls `{plugin}_crate_version` and verifies it
    /// matches the JS-side version.
    #[unsafe(no_mangle)]
    pub extern "C" fn mahjong_ws_crate_version() -> u32 {
        1
    }

    /// Reads the page-configured server URL (window.MAHJONG_SERVER_URL).
    pub fn page_server_url() -> Option<String> {
        let mut buf = vec![0u8; 1024];
        let len = unsafe { mahjong_ws_default_url(buf.as_mut_ptr(), buf.len()) };
        if len <= 0 {
            return None;
        }
        buf.truncate(len as usize);
        String::from_utf8(buf).ok()
    }

    /// WASM transport driving the ws.js WebSocket by handle.
    pub struct WasmTransport {
        handle: i32,
        /// Whether Opened has been delivered
        opened_reported: bool,
        /// Whether Closed/Error has been delivered (poll then stays empty)
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
            // Failures (not yet open / already closed) surface as a
            // status change in poll.
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

            // Dequeue received messages: length first, then copy.
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
                    Err(_) => events.push(WsEvent::Error("invalid UTF-8 from server".to_string())),
                }
            }

            // Deliver the terminal state only after draining
            // received messages.
            match status {
                STATUS_CLOSED => {
                    self.terminated = true;
                    events.push(WsEvent::Closed);
                }
                STATUS_ERROR => {
                    self.terminated = true;
                    events.push(WsEvent::Error("WebSocket error".to_string()));
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

#[cfg(test)]
mod tests {
    use super::url_with_room;

    #[test]
    fn test_url_with_room_appends_query() {
        assert_eq!(
            url_with_room("ws://127.0.0.1:8080/ws", "ABC234"),
            "ws://127.0.0.1:8080/ws?room=ABC234"
        );

        assert_eq!(
            url_with_room("wss://example.com/ws?v=1", "ABC234"),
            "wss://example.com/ws?v=1&room=ABC234"
        );
    }

    #[test]
    fn test_url_with_room_escapes_unsafe_chars() {
        // Escape characters that could break the URL structure.
        assert_eq!(
            url_with_room("ws://h/ws", "A&B=#?"),
            "ws://h/ws?room=A%26B%3D%23%3F"
        );
    }
}
