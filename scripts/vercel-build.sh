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
cp index.html public/index.html
sed -i 's|target/wasm32-unknown-unknown/release/mahjong-client.wasm|mahjong-client.wasm|' public/index.html
