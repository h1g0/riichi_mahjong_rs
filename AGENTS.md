# AGENTS.md

## Project Overview

Japanese Riichi Mahjong game implemented in Rust. Runs as a native desktop app and in the browser via WASM, with online play through a WebSocket server.

## Where the documentation lives

This file holds only what is specific to working as an agent here. Everything else has a single home; follow the link instead of relying on a copy:

| Topic | Document |
|---|---|
| Crates, module map, the `ServerEvent` / `ClientAction` seam, repository layout, largest files | [`docs/architecture.md`](docs/architecture.md) |
| Build / run / test commands, WASM constraints, pre-commit checks, branch and commit rules, comment and i18n conventions | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Mahjong terminology (Japanese / English / code identifiers) | [`docs/glossary.md`](docs/glossary.md) |
| Deployment (Vercel, Fly.io, itch.io) and server environment variables | [`README.md`](README.md) |

Japanese editions: [`docs/architecture.ja.md`](docs/architecture.ja.md), [`docs/CONTRIBUTING.ja.md`](docs/CONTRIBUTING.ja.md), [`docs/glossary.ja.md`](docs/glossary.ja.md), [`docs/README.ja.md`](docs/README.ja.md). They are translations, not separate sources of truth — when you change one edition, change the other in the same commit.

**`CONTRIBUTING.md` is normative for agents too.** Its rules on formatting, testing, branch naming, English commit messages, comment style, and i18n apply to every change you make, and are not repeated here.

## Working rules for agents

- **Start at the seam.** Before writing code, read the seam section of `docs/architecture.md`. Most features begin as a `ClientAction` or `ServerEvent` variant, and a change that bypasses the protocol is almost always in the wrong crate.
- **Check `mahjong-core` first.** Do not add game logic to `mahjong-server` or `mahjong-client` before confirming the core crate does not already provide it.
- **Verify before reporting done.** Run `cargo build && cargo test`, plus the `cargo fmt` / `cargo clippy` checks from `CONTRIBUTING.md`. Report failures rather than describing the change as complete.
- **Tests are not optional.** New functionality gets unit tests; every bug fix gets a regression test citing the issue number.
- **Keep the docs in step.** A change that moves a module, renames a protocol variant, or alters a documented command invalidates `docs/architecture.md` or `CONTRIBUTING.md` — update them in the same commit.

## GitHub CLI

`gh` is available at `/c/Program Files/GitHub CLI/gh.exe`. Add to PATH if needed:

```sh
export PATH="/c/Program Files/GitHub CLI:$PATH"
```
