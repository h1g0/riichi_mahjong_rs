# Contributing to Riichi Mahjong RS

[日本語版はこちら](./docs/CONTRIBUTING.ja.md)

Thanks for taking an interest in this project. It is a Japanese Riichi Mahjong
implementation in Rust: a rules engine, a CPU opponent, a Macroquad client that
runs natively and in the browser, and a WebSocket server for online play.

Bug reports, rule corrections, CPU-strategy improvements, translations, and
documentation fixes are all welcome. This document covers how to build, test,
and submit changes. For a map of the codebase, read
[docs/architecture.md](docs/architecture.md).

## Table of contents

- [Getting set up](#getting-set-up)
- [Build, run, and test](#build-run-and-test)
- [Before you commit](#before-you-commit)
- [Branches, commits, and pull requests](#branches-commits-and-pull-requests)
- [Coding conventions](#coding-conventions)
- [Good first issues](#good-first-issues)
- [Where to ask questions](#where-to-ask-questions)

## Getting set up

You need a recent stable Rust toolchain. Install it with
[rustup](https://rustup.rs) if you do not have one.

```sh
rustc --version
cargo --version
```

Clone the repository:

```sh
git clone https://github.com/h1g0/riichi_mahjong_rs.git
cd riichi_mahjong_rs
```

The native client and the servers need nothing else. For the browser (WASM)
build you also need the WASM target, plus Bash with the standard Unix utilities
and Python 3:

```sh
rustup target add wasm32-unknown-unknown
```

## Build, run, and test

### Tests

```sh
cargo test
```

Most work happens in one crate at a time, and a scoped run is much faster:

```sh
cargo test -p mahjong-core
cargo test -p mahjong-server
```

CI runs `cargo test --workspace --all-targets --all-features --locked`, so a
change that passes locally with the default flags can still fail there if it
breaks an example or an integration test.

### The native client

```sh
cargo run -p mahjong-client
```

Pressing `F12` saves a PNG screenshot to `screenshots/`.

### The browser client

```sh
bash scripts/vercel-build.sh
```

```sh
python -m http.server 8080 --directory public
```

Then open <http://127.0.0.1:8080>.

The script builds `mahjong-client` for `wasm32-unknown-unknown`, copies
`mq_js_bundle.js` from the Macroquad version that `Cargo.lock` resolved, and
assembles everything under `public/`. `public/` is generated output — do not
hand-edit it; edit `assets/web/` instead.

Two WASM constraints are deliberate and easy to break by accident:

- `.cargo/config.toml` sets `getrandom_backend="custom"`, because Miniquad's
  WASM loader does not use wasm-bindgen.
  `crates/mahjong-client/src/wasm_rng.rs` supplies that backend.
- **Do not add a wasm-bindgen dependency.** Browser APIs are reached through
  hand-written JavaScript glue shipped with the web assets.

### The online server

```sh
cargo run -p mahjong-net-server
```

It listens on `PORT` (default `8080`), serves the WebSocket endpoint at `/ws`,
and answers `GET /healthz`. To point a native client at it:

```sh
MAHJONG_SERVER_URL=ws://127.0.0.1:8080/ws cargo run -p mahjong-client
```

Note that when `ALLOWED_ORIGINS` (or the legacy `ALLOWED_ORIGIN`) is set, native
clients are rejected with HTTP 403, because they send no `Origin` header. Leave
both unset for local development. The full list of server environment variables
is in the [README](README.md#online-multiplayer-server).

### CPU simulations

Changes to the CPU AI should be checked against a batch of games rather than a
single one:

```sh
cargo run -p mahjong-server --release --example cpu_simulation -- 100 42 2>/dev/null
```

The arguments are the number of games and the base seed. Results are
deterministic for a given seed and dependency set, so running this before and
after a heuristics change and comparing win rates, deal-in rates, and average
placement works as a regression check. Quote the before/after numbers in the
pull request.

### mjai tooling

The CPU opponent can be run as an [mjai](https://mjai.app) bot, and games can be
exported as mjai logs for review by existing tools:

```sh
cargo run -p mahjong-mjai --bin mjai-bot -- --level strong --name my-bot
```

```sh
cargo run -p mahjong-mjai --example mjai_export -- 42 > game.mjson
```

Games from elsewhere go the other way. Convert a Tenhou or Mahjong Soul record
into mjai with one of the existing converters, then replay it through the
engine, which compares every result with the one the log reports:

```sh
cargo run -p mahjong-mjai --bin mjai-import -- --rules tenhou game.mjson
```

It exits non-zero when the log and this project disagree on han, minipoints,
payments, or who was ready at an exhaustive draw, so a batch of logs works as a
regression check on the rules engine. Because mjai carries no rule set, pass the
`--rules` the log was actually played under; a disagreement caused by the wrong
rule set looks exactly like a bug.

## Before you commit

Both of these must be clean; CI fails on either:

```sh
cargo fmt
```

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also:

- **Add tests for new functionality.** Unit tests live next to the code they
  cover (a `mod tests` in the same file, or a sibling `*_tests.rs`).
- **Add a regression test for every bug fix**, and mention the issue number in
  the test name or a comment so the next reader knows what it guards.
- **Check `mahjong-core` first.** Before adding game logic to `mahjong-server`
  or `mahjong-client`, make sure it does not already exist in the core crate.
  Shanten, yaku, fu, and score calculation belong there, not in the callers.

## Branches, commits, and pull requests

Branch names follow `{type}/{issue-number}-{english-branch-name}`:

- `fix` — a bug fix
- `feat` — a new feature
- `misc` — anything else (docs, refactors, tooling)

For example, `fix/#87-hide-dora-on-draw-result`.

Commit messages and pull request descriptions must be written in **English**,
even though much of the domain vocabulary is Japanese. Explain **why** the
change is being made; the code already says what it does.

A good pull request:

- targets `main` and stays focused on one issue,
- links the issue it addresses,
- says how it was verified (which tests, which manual steps, and simulation
  numbers for CPU changes),
- includes a screenshot for anything that changes the UI.

## Coding conventions

### Terminology

Riichi Mahjong terms are fixed by the
[Ubiquitous Language Glossary](docs/glossary.md)
([Japanese edition](docs/glossary.ja.md)). English names follow the World Riichi
Championship (WRC) Rules 2025. Use those names in code, comments, and
documentation, and extend the glossary if you introduce a term it does not
cover.

Where recognition is helped by it, append the Japanese in parentheses:
`liability payment (pao / 包)`.

### Comments

The project follows one maxim: code says **how**, tests say **what**, commit
logs say **why**, and code comments say **why not**.

- Non-doc comments (`//`) must carry a constraint, a rejected simpler
  alternative, or a bug being avoided — cite the issue number, e.g. `#294`.
  Comments that restate what the code plainly does get deleted.
- Doc comments (`///`, `//!`) document the API: purpose, invariants, units, and
  edge cases. Keep them even when they are short. A `//!` header on every module
  is the convention here, and it is what makes the codebase navigable.
- Write all comments in English.
- **Do not translate or alter Japanese inside string literals** — UI text, i18n
  strings, and test data are content, not commentary.

### User-visible text

The client UI is multilingual (Japanese and English). Fixed strings live in the
`Key` enum in `crates/mahjong-client/src/i18n/mod.rs`, which requires every
language to be filled in per key, so a missing translation is a compile error
rather than a silent fallback. Do not hard-code a display string in the
renderer.

## Good first issues

Issues labelled
[`good first issue`](https://github.com/h1g0/riichi_mahjong_rs/labels/good%20first%20issue)
are self-contained: they touch a small number of files and do not require
knowing the rules engine as a whole. `documentation` and `help wanted` are worth
a look too.

If nothing is labelled at the moment, open an issue describing what you would
like to work on — that is also the best way to check that nobody else has
started. Please claim an issue by commenting on it before opening a pull request
for it.

## Where to ask questions

- **Bugs and feature requests**: open an
  [issue](https://github.com/h1g0/riichi_mahjong_rs/issues/new/choose).
- **Questions about the codebase, a rules interpretation, or how to approach a
  change**: open an issue with the `question` label, or ask in the issue you are
  working on. Japanese and English are both fine — the maintainer reads both.
  Only committed artefacts (commit messages, pull request descriptions, comments
  in code) have to be in English.

By contributing, you agree that your contributions are licensed under this
project's [MIT License](LICENSE).
