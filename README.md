# riichi_mahjong_rs

[![Build and test](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml/badge.svg?branch=main)](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml)

Implementation for Japanese Riichi Mahjong in Rust.

Currently,

- [x] Implementation of calculating Shanten number (Number showing the minimum number of tile changes to win.) is completed.
- [x] Implementation of [Yaku](https://en.wikipedia.org/wiki/Japanese_Mahjong_yaku) (winning hand) evaluation.
- [x] Implementation of score calculation.

麻雀のRustでの実装

現在は

- [x] シャンテン数の計算を実装完了
- [x] 役の判定を実装完了
- [x] 符計算および点数計算を実装完了

## Vercel deployment

This project can be built on Vercel instead of committing prebuilt WASM assets.

1. Import the repository into Vercel.
2. Keep the project root at this repository root.
3. Deploy. Vercel will run `bash scripts/vercel-install.sh` and `bash scripts/vercel-build.sh` from `vercel.json`.

The build does the following:
- installs `rustup` if needed
- adds the `wasm32-unknown-unknown` target
- builds `mahjong-client` in release mode
- downloads `mq_js_bundle.js`
- writes deployable assets to `public/`

If you want to test the same flow locally, run the equivalent steps manually on a machine with Bash, curl, Rust, and the wasm target installed.

