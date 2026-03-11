# riichi_mahjong_rs

[日本語版はこちら](README.ja.md)

[![Build and test](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml/badge.svg?branch=main)](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml)

Implementation for Japanese Riichi Mahjong in Rust.

## Current status

- Shanten calculation is implemented.
- [Yaku](https://en.wikipedia.org/wiki/Japanese_Mahjong_yaku) evaluation is implemented.
- Fu calculation and score calculation are implemented.
- A playable client that runs in both native and WASM builds is included.
  - The current client is a temporary simplified version.
  - CPU opponent algorithms are planned for future implementation. At present, seats other than the player only draw and discard the drawn tile.
  - The design also takes future networked multiplayer implementation into consideration.
- Vercel deployment using the included scripts is supported.

## Structure

### Crate structure

This repository is currently composed of the following crates.

- `mahjong-core`: core logic such as hand representation, shanten calculation, yaku evaluation, fu calculation, and score calculation
- `mahjong-server`: progression management and rule handling used for local matches
- `mahjong-client`: a Macroquad-based four-player Riichi Mahjong client that supports both native and browser execution

### Directory structure

- `crates/`: workspace crates
- `assets/`: runtime assets such as fonts
- `public/`: web assets for deployment
- `scripts/`: build scripts used for deployment
- `index.html`: local web entry point for the WASM client
- `vercel.json`: Vercel build configuration

## Development

First, make sure that the latest stable Rust compiler and Cargo are installed.

~~~sh
rustc --version
cargo --version
~~~

If Rust or Cargo is not installed, install them using [rustup](https://rustup.rs) and follow the setup instructions for your platform.

Then clone the repository and move into the project directory.

~~~sh
git clone git@github.com:h1g0/riichi_mahjong_rs.git
cd riichi_mahjong_rs
~~~

If you want to run the project locally with WASM, add the WASM target.

~~~sh
rustup target add wasm32-unknown-unknown
~~~

### Commands

Run tests:

~~~sh
cargo test
~~~

Run the native client locally:

~~~sh
cargo run -p mahjong-client
~~~

Build the browser client locally:

~~~sh
cargo build -p mahjong-client --target wasm32-unknown-unknown --release
~~~

After building, serve this repository with any local static file server you prefer and open index.html to view the generated WASM client in a browser.

e.g.

If `npx` is installed:

~~~sh
npx serve .
~~~

If Python is installed:

~~~sh
python -m http.server 8080
~~~

## Vercel deployment

This project is set up so it can be built on Vercel without committing generated WASM artifacts for every deployment.

1. Import the repository into Vercel.
2. Keep the project root as the root of this repository.
3. When you deploy, the following commands will be run according to `vercel.json`.

~~~sh
bash scripts/vercel-install.sh
bash scripts/vercel-build.sh
~~~

The Vercel build performs the following steps.

- installs `rustup` when necessary
- adds the `wasm32-unknown-unknown` target
- builds `mahjong-client` in release mode
- places deployable web assets under `public/`

To reproduce the same flow locally, run equivalent steps in an environment where Bash, curl, Rust, and the WASM target are available.
