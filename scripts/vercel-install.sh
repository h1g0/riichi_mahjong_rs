#!/usr/bin/env bash
set -euo pipefail

export HOME="${HOME:-/vercel}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"

if ! command -v curl >/dev/null 2>&1; then
  dnf install -y curl
fi

if ! command -v rustup >/dev/null 2>&1; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
fi

if [ -f "$CARGO_HOME/env" ]; then
  source "$CARGO_HOME/env"
else
  export PATH="$CARGO_HOME/bin:$PATH"
fi

rustup target add wasm32-unknown-unknown
