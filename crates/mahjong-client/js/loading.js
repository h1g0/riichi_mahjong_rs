// Startup loading-overlay plugin.
//
// The HTML overlay is visible before WASM exists. Rust hides it only after
// Macroquad has presented its own loading frame, avoiding a black gap between
// browser startup and synchronous asset initialization.
"use strict";

const MAHJONG_LOADING_VERSION = 1;

function mahjong_loading_hide() {
    const overlay = document.getElementById("loading");
    if (overlay) {
        overlay.hidden = true;
    }
}

miniquad_add_plugin({
    name: "mahjong_loading",
    version: MAHJONG_LOADING_VERSION,
    register_plugin(importObject) {
        importObject.env.mahjong_loading_hide = mahjong_loading_hide;
    },
});
