// オンライン対戦用 WebSocket プラグイン
//
// miniquad の mq_js_bundle.js が提供するプラグイン機構で WASM に
// WebSocket 機能を注入する。wasm-bindgen は使わない（wasm_rng.rs と同じ方針）。
//
// Rust 側 (crates/mahjong-client/src/transport.rs) の extern "C" 宣言と
// 関数シグネチャ・ステータス値を一致させること。
"use strict";

const MAHJONG_WS_VERSION = 1;

// 接続ステータス（Rust 側と一致させる）
// 0 = 接続中, 1 = 接続済み, 2 = 切断, 3 = エラー
const MAHJONG_WS_CONNECTING = 0;
const MAHJONG_WS_OPEN = 1;
const MAHJONG_WS_CLOSED = 2;
const MAHJONG_WS_ERROR = 3;

const mahjong_ws = {
    // handle (配列インデックス) -> { ws, status, queue: [Uint8Array] }
    sockets: [],
    encoder: new TextEncoder(),
    decoder: new TextDecoder(),

    read_str(ptr, len) {
        return this.decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len));
    },
};

// 接続を開始し、ハンドルを返す
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

// 接続ステータスを返す
function mahjong_ws_status(handle) {
    const entry = mahjong_ws.sockets[handle];
    return entry ? entry.status : MAHJONG_WS_ERROR;
}

// テキストフレームを送信する（成功 0 / 失敗 -1）
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

// 受信キュー先頭のメッセージのバイト長を返す（空なら -1）
function mahjong_ws_next_msg_len(handle) {
    const entry = mahjong_ws.sockets[handle];
    if (!entry || entry.queue.length === 0) {
        return -1;
    }
    return entry.queue[0].length;
}

// 受信キュー先頭のメッセージを buf_ptr へコピーして取り除く
// （Rust 側が mahjong_ws_next_msg_len の長さでバッファを確保して呼ぶ）
function mahjong_ws_read_msg(handle, buf_ptr) {
    const entry = mahjong_ws.sockets[handle];
    if (!entry || entry.queue.length === 0) {
        return;
    }
    const msg = entry.queue.shift();
    new Uint8Array(wasm_memory.buffer, buf_ptr, msg.length).set(msg);
}

// 接続を閉じる
function mahjong_ws_close(handle) {
    const entry = mahjong_ws.sockets[handle];
    if (entry && entry.ws) {
        try {
            entry.ws.close();
        } catch (_err) {
            // 既に閉じている場合などは無視
        }
    }
    if (entry && entry.status !== MAHJONG_WS_ERROR) {
        entry.status = MAHJONG_WS_CLOSED;
    }
}

// ページに設定された接続先URL (window.MAHJONG_SERVER_URL) を buf_ptr に書き込み、
// バイト長を返す（未設定・容量不足なら 0 を返し、Rust 側が既定値を使う）
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
