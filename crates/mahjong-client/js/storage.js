// Settings persistence plugin (localStorage).
//
// Injects localStorage access into the WASM module via miniquad's
// mq_js_bundle.js plugin mechanism; no wasm-bindgen (same policy as
// ws.js).
//
// Function signatures and value meanings must match the extern "C"
// declarations in crates/mahjong-client/src/persistence.rs.
"use strict";

const MAHJONG_STORAGE_VERSION = 1;

// Language codes, matching the Rust side: -1 = unset, 0 = Japanese,
// 1 = English
const MAHJONG_LANG_KEY = "mahjong.lang";

function mahjong_storage_browser_lang() {
    let languages = [];
    if (typeof navigator !== "undefined") {
        if (Array.isArray(navigator.languages) && navigator.languages.length > 0) {
            languages = navigator.languages;
        } else if (typeof navigator.language === "string") {
            languages = [navigator.language];
        }
    }

    const hasJapanese = languages.some((language) => {
        if (typeof language !== "string") {
            return false;
        }
        const normalized = language.toLowerCase();
        return normalized === "ja" || normalized.startsWith("ja-");
    });
    return hasJapanese ? 0 : 1;
}

function mahjong_storage_get_lang() {
    try {
        const v = window.localStorage.getItem(MAHJONG_LANG_KEY);
        if (v === "ja") {
            return 0;
        }
        if (v === "en") {
            return 1;
        }
    } catch (_err) {
        // Browser detection keeps first launch usable when storage is blocked.
    }
    return mahjong_storage_browser_lang();
}

// Saves the display language (0 = Japanese, 1 = English; others ignored).
function mahjong_storage_set_lang(code) {
    try {
        if (code === 0) {
            window.localStorage.setItem(MAHJONG_LANG_KEY, "ja");
        } else if (code === 1) {
            window.localStorage.setItem(MAHJONG_LANG_KEY, "en");
        }
    } catch (_err) {
        // Silently ignore environments that cannot persist.
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
