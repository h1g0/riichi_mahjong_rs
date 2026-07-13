// WebSocket plugin for online play.
//
// Injects WebSocket support into the WASM module via miniquad's
// mq_js_bundle.js plugin mechanism; no wasm-bindgen (same policy as
// wasm_rng.rs).
//
// Function signatures and status values must match the extern "C"
// declarations in crates/mahjong-client/src/transport.rs.
"use strict";

const MAHJONG_WS_VERSION = 1;

// Connection status, matching the Rust side:
// 0 = connecting, 1 = connected, 2 = closed, 3 = error
const MAHJONG_WS_CONNECTING = 0;
const MAHJONG_WS_OPEN = 1;
const MAHJONG_WS_CLOSED = 2;
const MAHJONG_WS_ERROR = 3;

const mahjong_ws = {
    // handle (array index) -> { ws, status, queue: [Uint8Array] }
    sockets: [],
    encoder: new TextEncoder(),
    decoder: new TextDecoder(),

    read_str(ptr, len) {
        return this.decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len));
    },
};

// Opens a connection and returns its handle.
function mahjong_ws_connect(url_ptr, url_len) {
    const url = mahjong_ws.read_str(url_ptr, url_len);
    const handle = mahjong_ws.sockets.length;
    const entry = { ws: null, status: MAHJONG_WS_CONNECTING, queue: [] };
    mahjong_ws.sockets.push(entry);

    try {
        const ws = new WebSocket(url);
        entry.ws = ws;
        ws.onopen = () => {
            entry.status = MAHJONG_WS_OPEN;
        };
        ws.onmessage = (event) => {
            if (typeof event.data === "string") {
                entry.queue.push(mahjong_ws.encoder.encode(event.data));
            }
        };
        ws.onclose = () => {
            if (entry.status !== MAHJONG_WS_ERROR) {
                entry.status = MAHJONG_WS_CLOSED;
            }
        };
        ws.onerror = (event) => {
            console.error("mahjong_ws: WebSocketエラー", event);
            entry.status = MAHJONG_WS_ERROR;
        };
    } catch (err) {
        console.error("mahjong_ws: 接続に失敗しました", err);
        entry.status = MAHJONG_WS_ERROR;
    }

    return handle;
}

// Returns the connection status.
function mahjong_ws_status(handle) {
    const entry = mahjong_ws.sockets[handle];
    return entry ? entry.status : MAHJONG_WS_ERROR;
}

// Sends a text frame; 0 on success, -1 on failure.
function mahjong_ws_send(handle, ptr, len) {
    const entry = mahjong_ws.sockets[handle];
    if (!entry || entry.status !== MAHJONG_WS_OPEN) {
        return -1;
    }
    try {
        entry.ws.send(mahjong_ws.read_str(ptr, len));
        return 0;
    } catch (err) {
        console.error("mahjong_ws: 送信に失敗しました", err);
        entry.status = MAHJONG_WS_ERROR;
        return -1;
    }
}

// Byte length of the front queued message; -1 when empty.
function mahjong_ws_next_msg_len(handle) {
    const entry = mahjong_ws.sockets[handle];
    if (!entry || entry.queue.length === 0) {
        return -1;
    }
    return entry.queue[0].length;
}

// Copies the front queued message into buf_ptr and dequeues it. Rust
// sizes the buffer from mahjong_ws_next_msg_len before calling.
function mahjong_ws_read_msg(handle, buf_ptr) {
    const entry = mahjong_ws.sockets[handle];
    if (!entry || entry.queue.length === 0) {
        return;
    }
    const msg = entry.queue.shift();
    new Uint8Array(wasm_memory.buffer, buf_ptr, msg.length).set(msg);
}

// Closes the connection and frees its resources. Each reconnect mints a
// new handle, so stale references and queues must not linger.
function mahjong_ws_close(handle) {
    const entry = mahjong_ws.sockets[handle];
    if (!entry) {
        return;
    }
    if (entry.ws) {
        try {
            entry.ws.close();
        } catch (_err) {
            // Ignore errors from an already-closed socket.
        }
        entry.ws.onopen = entry.ws.onmessage = entry.ws.onclose = entry.ws.onerror = null;
        entry.ws = null;
    }
    entry.queue = [];
    if (entry.status !== MAHJONG_WS_ERROR) {
        entry.status = MAHJONG_WS_CLOSED;
    }
}

// Writes the page-configured server URL (window.MAHJONG_SERVER_URL) into
// buf_ptr and returns its byte length; 0 when unset or too large, in
// which case Rust falls back to its default.
function mahjong_ws_default_url(buf_ptr, cap) {
    const url = typeof window !== "undefined" && window.MAHJONG_SERVER_URL;
    if (!url) {
        return 0;
    }
    const bytes = mahjong_ws.encoder.encode(url);
    if (bytes.length > cap) {
        console.error("mahjong_ws: MAHJONG_SERVER_URL が長すぎます");
        return 0;
    }
    new Uint8Array(wasm_memory.buffer, buf_ptr, bytes.length).set(bytes);
    return bytes.length;
}

miniquad_add_plugin({
    name: "mahjong_ws",
    version: MAHJONG_WS_VERSION,
    register_plugin(importObject) {
        importObject.env.mahjong_ws_connect = mahjong_ws_connect;
        importObject.env.mahjong_ws_status = mahjong_ws_status;
        importObject.env.mahjong_ws_send = mahjong_ws_send;
        importObject.env.mahjong_ws_next_msg_len = mahjong_ws_next_msg_len;
        importObject.env.mahjong_ws_read_msg = mahjong_ws_read_msg;
        importObject.env.mahjong_ws_close = mahjong_ws_close;
        importObject.env.mahjong_ws_default_url = mahjong_ws_default_url;
    },
});
