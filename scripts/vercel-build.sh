#!/usr/bin/env bash
set -euo pipefail

export HOME="${HOME:-/vercel}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"

if [ -f "$CARGO_HOME/env" ]; then
  source "$CARGO_HOME/env"
else
  export PATH="$CARGO_HOME/bin:$PATH"
fi

rm -rf public
mkdir -p public

cargo build --release --target wasm32-unknown-unknown -p mahjong-client
curl -L https://not-fl3.github.io/miniquad-samples/mq_js_bundle.js -o public/mq_js_bundle.js
cp target/wasm32-unknown-unknown/release/mahjong-client.wasm public/mahjong-client.wasm
cp crates/mahjong-client/js/ws.js public/ws.js
cp crates/mahjong-client/js/storage.js public/storage.js
cp index.html public/index.html

# Rename assets with a content hash so browsers can cache them immutably
# (see the Cache-Control headers in vercel.json) without ever serving a
# stale version after a new deployment.
wasm_hash=$(sha1sum public/mahjong-client.wasm | cut -c1-8)
js_hash=$(sha1sum public/mq_js_bundle.js | cut -c1-8)
ws_hash=$(sha1sum public/ws.js | cut -c1-8)
storage_hash=$(sha1sum public/storage.js | cut -c1-8)
mv public/mahjong-client.wasm "public/mahjong-client.${wasm_hash}.wasm"
mv public/mq_js_bundle.js "public/mq_js_bundle.${js_hash}.js"
mv public/ws.js "public/ws.${ws_hash}.js"
mv public/storage.js "public/storage.${storage_hash}.js"

sed -i "s|target/wasm32-unknown-unknown/release/mahjong-client.wasm|mahjong-client.${wasm_hash}.wasm|" public/index.html
sed -i "s|mq_js_bundle.js|mq_js_bundle.${js_hash}.js|" public/index.html
sed -i "s|crates/mahjong-client/js/ws.js|ws.${ws_hash}.js|" public/index.html
sed -i "s|crates/mahjong-client/js/storage.js|storage.${storage_hash}.js|" public/index.html

# The online game server URL for the deployed client. Set the
# MAHJONG_SERVER_URL environment variable in the Vercel project to
# point the web client at the production server (e.g. wss://.../ws).
if [ -n "${MAHJONG_SERVER_URL:-}" ]; then
  sed -i "s|// window.MAHJONG_SERVER_URL = .*|window.MAHJONG_SERVER_URL = \"${MAHJONG_SERVER_URL}\";|" public/index.html
fi
