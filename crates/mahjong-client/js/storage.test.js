"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const storagePluginSource = fs.readFileSync(
    path.join(__dirname, "storage.js"),
    "utf8",
);

function loadLanguage({ saved = null, languages, language, storageError = false }) {
    let plugin;
    const context = {
        window: {
            localStorage: {
                getItem() {
                    if (storageError) {
                        throw new Error("localStorage unavailable");
                    }
                    return saved;
                },
                setItem() {},
            },
        },
        navigator: { languages, language },
        miniquad_add_plugin(registeredPlugin) {
            plugin = registeredPlugin;
        },
    };
    vm.runInNewContext(storagePluginSource, context);

    const importObject = { env: {} };
    plugin.register_plugin(importObject);
    return importObject.env.mahjong_storage_get_lang();
}

test("a saved language takes precedence over browser preferences", () => {
    assert.equal(loadLanguage({ saved: "ja", languages: ["en-US"] }), 0);
    assert.equal(loadLanguage({ saved: "en", languages: ["ja-JP"] }), 1);
});

test("Japanese anywhere in the browser language list selects Japanese", () => {
    assert.equal(loadLanguage({ languages: ["en-US", "ja-JP"] }), 0);
    assert.equal(loadLanguage({ languages: ["JA"] }), 0);
});

test("browser preferences without Japanese select English", () => {
    assert.equal(loadLanguage({ languages: ["en-US", "fr-FR"] }), 1);
    assert.equal(loadLanguage({ languages: ["javanese"] }), 1);
});

test("navigator.language is used when navigator.languages is unavailable", () => {
    assert.equal(loadLanguage({ language: "ja-JP" }), 0);
    assert.equal(loadLanguage({ language: "en-US" }), 1);
});

test("browser preferences remain available when localStorage throws", () => {
    assert.equal(loadLanguage({ languages: ["ja"], storageError: true }), 0);
});
