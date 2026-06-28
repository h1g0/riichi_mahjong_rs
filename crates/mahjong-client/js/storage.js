// 設定の永続化プラグイン（localStorage）
//
// miniquad の mq_js_bundle.js が提供するプラグイン機構で WASM に
// localStorage アクセスを注入する。wasm-bindgen は使わない（ws.js と同じ方針）。
//
// Rust 側 (crates/mahjong-client/src/persistence.rs) の extern "C" 宣言と
// 関数シグネチャ・値の意味を一致させること。
"use strict";

const MAHJONG_STORAGE_VERSION = 1;

// 言語コード（Rust 側と一致させる）: -1 = 未設定, 0 = 日本語, 1 = 英語
const MAHJONG_LANG_KEY = "mahjong.lang";

// 保存された表示言語を返す（未設定・不正値なら -1）
function mahjong_storage_get_lang() {
    try {
        const v = window.localStorage.getItem(MAHJONG_LANG_KEY);
        if (v === "ja") {
            return 0;
        }
        if (v === "en") {
            return 1;
        }
        return -1;
    } catch (_err) {
        // localStorage が使えない環境（プライベートモード等）では未設定扱い
        return -1;
    }
}

// 表示言語を保存する（0 = 日本語, 1 = 英語、その他は無視）
function mahjong_storage_set_lang(code) {
    try {
        if (code === 0) {
            window.localStorage.setItem(MAHJONG_LANG_KEY, "ja");
        } else if (code === 1) {
            window.localStorage.setItem(MAHJONG_LANG_KEY, "en");
        }
    } catch (_err) {
        // 保存できない環境では黙って無視する
    }
}

miniquad_add_plugin({
    name: "mahjong_storage",
    version: MAHJONG_STORAGE_VERSION,
    register_plugin(importObject) {
        importObject.env.mahjong_storage_get_lang = mahjong_storage_get_lang;
        importObject.env.mahjong_storage_set_lang = mahjong_storage_set_lang;
    },
});
