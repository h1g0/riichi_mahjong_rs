//! Browser plugin registration regression tests (#329).

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');

test('the browser bootstrap loads the bundled and application plugins', () => {
    const root = path.resolve(__dirname, '..');
    const html = fs.readFileSync(path.join(root, 'assets/web/index.html'), 'utf8');
    const canvas = { style: {}, addEventListener() {}, focus() {} };
    const context = vm.createContext({
        document: {
            querySelector() { return canvas; },
            getElementById() { return canvas; },
            addEventListener() {},
        },
        window: { innerWidth: 1280, innerHeight: 800, addEventListener() {} },
        navigator: { userAgent: 'bootstrap-test', platform: 'Win32' },
        console,
        TextEncoder,
        TextDecoder,
        WebAssembly,
        performance: { now() { return 0; } },
    });

    for (const script of html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g)) {
        const src = /src="([^"]+)"/.exec(script[1])?.[1];
        if (src === 'mq_js_bundle.js') {
            // Node's VM permits undeclared function assignments in strict mode;
            // a scalar probe enforces the browser's declared-binding rule (#329).
            vm.runInContext('"use strict"; register_plugin = null;', context);
            vm.runInContext(fs.readFileSync(path.join(root, 'public', src), 'utf8'), context);
        } else if (src) {
            vm.runInContext(fs.readFileSync(path.join(root, 'crates/mahjong-client/js', src), 'utf8'), context);
        } else if (!script[2].includes('load("mahjong-client.wasm")')) {
            vm.runInContext(script[2], context);
        }
    }
    vm.runInContext('register_plugins(plugins)', context);
    for (const name of ['quad_net', 'mahjong_ws', 'mahjong_storage', 'mahjong_loading']) {
        assert.ok(context.plugins.some(plugin => plugin.name === name), `${name} was not registered`);
    }
    assert.equal(typeof context.importObject.env.mahjong_ws_connect, 'function');
});
