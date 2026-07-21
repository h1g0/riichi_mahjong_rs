"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const loadingPluginSource = fs.readFileSync(
    path.join(__dirname, "loading.js"),
    "utf8",
);
const startupPageSource = fs.readFileSync(
    path.join(__dirname, "../../../assets/web/index.html"),
    "utf8",
);

function loadPlugin(overlay) {
    let plugin;
    const context = {
        document: {
            getElementById(id) {
                return id === "loading" ? overlay : null;
            },
        },
        miniquad_add_plugin(registeredPlugin) {
            plugin = registeredPlugin;
        },
    };
    vm.runInNewContext(loadingPluginSource, context);

    const importObject = { env: {} };
    plugin.register_plugin(importObject);
    return importObject.env.mahjong_loading_hide;
}

test("the WASM callback hides the startup overlay", () => {
    const overlay = { hidden: false };
    loadPlugin(overlay)();
    assert.equal(overlay.hidden, true);
});

test("the WASM callback tolerates a missing startup overlay", () => {
    assert.doesNotThrow(() => loadPlugin(null)());
});

test("the startup overlay obscures the canvas until WASM hides it", () => {
    assert.match(
        startupPageSource,
        /#loading\s*\{[^}]*background:\s*#060e09\s*;/s,
    );
});
