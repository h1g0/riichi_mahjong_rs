# Architecture

[日本語版はこちら](./architecture.ja.md)

A map of the codebase for people arriving cold. It is roughly 57k lines of Rust
across five crates, but almost all of it hangs off one seam — the event stream
described in [The seam](#the-seam-serverevent--clientaction). Read that section
first; the rest is detail you can look up when you need it.

For how to build, test, and submit changes, see
[CONTRIBUTING.md](../CONTRIBUTING.md). For terminology, see the
[Ubiquitous Language Glossary](glossary.md).

## The crates

```
mahjong-core          rules: tiles, hands, shanten, yaku, fu, scoring
   ^
   |
mahjong-server        game progression: wall, table, round, legality, CPU
   ^        ^      ^
   |        |      |
client   net-server  mjai
```

| Crate | Owns | Depends on |
|---|---|---|
| [`mahjong-core`](../crates/mahjong-core) | Pure rules. Tile representation, hand analysis, shanten, yaku evaluation, fu and score calculation. No I/O, no game flow, no UI. | — |
| [`mahjong-server`](../crates/mahjong-server) | Game progression: the wall, a table across a whole game, one hand at a time, which moves are legal, and the CPU opponent. Synchronous and I/O-free. | core |
| [`mahjong-client`](../crates/mahjong-client) | The Macroquad GUI, native and WASM. Menus, board rendering, input, i18n. | core, server |
| [`mahjong-net-server`](../crates/mahjong-net-server) | Online play: a WebSocket server with room-code lobbies, one tokio task per room. | server |
| [`mahjong-mjai`](../crates/mahjong-mjai) | Translation to and from the [mjai](https://mjai.app) protocol, so this project's CPU can be run and reviewed by existing mahjong AI tooling. | core, server (optional) |

Two rules follow from that layout, and most review comments on new code come
back to them:

- **Rules go in `mahjong-core`.** Before adding game logic to the server or the
  client, check whether the core crate already has it.
- **`mahjong-server` is synchronous, does no network or file I/O, and reads no
  clock.** Time is injected as a `now: f64` parameter. That is why the same code
  drives a macroquad frame loop, an async server, and a batch simulation without
  changes.

## Repository layout

| Path | Contents |
|---|---|
| `crates/` | The workspace crates above. |
| `assets/fonts/` | ShipporiMincho-Regular.ttf, loaded at runtime (SIL OFL; see `ShipporiMincho-OFL.txt`). |
| `assets/images/` | Tile and score-stick PNGs, embedded into the client binary. |
| `assets/web/` | Source HTML and favicon for the browser client. |
| `crates/mahjong-client/js/` | Hand-written JavaScript glue for the WASM build: `ws.js` (WebSocket), `storage.js` (settings), `loading.js`. |
| `public/` | Generated web output. Built by `scripts/vercel-build.sh`; never edit by hand. |
| `scripts/` | Vercel build and install scripts, plus the asset helper scripts they call. |
| `docs/` | This guide, the glossary, the Japanese README, and images. |
| `Dockerfile`, `fly.toml` | Container and Fly.io configuration for `mahjong-net-server`. |
| `vercel.json` | Vercel build configuration for the web client. |

Deployment (Vercel for the web client, Fly.io or any Docker host for the online
server, itch.io from CI) is documented in the [README](../README.md).

## The seam: `ServerEvent` / `ClientAction`

Everything above `mahjong-server` talks to it through two enums in
[`protocol/mod.rs`](../crates/mahjong-server/src/protocol/mod.rs):

- **`ServerEvent`** — what a seat is told: `GameStarted`, `TileDrawn`,
  `TileDiscarded`, `CallAvailable`, `PlayerCalled`, `PlayerRiichi`,
  `RoundWon`, `RoundDraw`, and so on. Events are **per seat** and carry only
  what that seat is entitled to see (`OtherPlayerDrew` says a tile was drawn,
  not which one).
- **`ClientAction`** — what a seat declares: `Discard`, `Riichi`, `Chi`, `Pon`,
  `Kan`, `Pei`, `Tsumo`, `Ron`, `Pass`, `NineTerminals`.

[`GameDriver`](../crates/mahjong-server/src/driver.rs) is the pump. It owns a
`Table` plus the CPU clients, and exposes essentially two calls:

```rust
let events: Vec<ServerEvent> = driver.drain_events(seat);
let accepted: bool = driver.handle_action(seat, action);
```

Both are also available in a `*_at(.., now: f64)` form used when CPU "thinking
time" is enabled.

Inside `drain_events`, the driver pumps: it drains the table's queued
`(seat, event)` pairs, buffers the ones belonging to human seats, hands the rest
to the CPU clients, applies whatever actions those CPUs return, and loops,
because a CPU action generates further events. The consequence worth
internalizing is that **CPU players are not special**. They receive the same
`ServerEvent`s a human does and reply with the same `ClientAction`s; a CPU that
could see hidden state would have to be given it explicitly, and nothing does.

This is why the same driver serves three very different hosts:

| Host | Drives the driver by | File |
|---|---|---|
| Local play | a macroquad frame calling `tick_at` / `drain_events_at` with `get_time()` | [`adapter/local.rs`](../crates/mahjong-client/src/adapter/local.rs) |
| Online play | a tokio room task calling `run_until_blocked` and `drain_all_events_at` | [`net-server/src/room.rs`](../crates/mahjong-net-server/src/room.rs) |
| mjai | a decoder rebuilding the `ServerEvent` stream from mjai JSON | [`mjai/src/decode.rs`](../crates/mahjong-mjai/src/decode.rs) |

**If you are adding a feature, this is where it starts.** A new declaration is a
`ClientAction` variant; new information for players is a `ServerEvent` variant.
Adding either means touching the round logic that emits or accepts it, the CPU
client that must not choke on it, the client renderer, and — because the online
protocol serializes these enums as JSON — the mjai codec if it is representable
there. Fields added to an existing variant should carry `#[serde(default)]` so
that older peers stay decodable.

## Following one hand

Reading order for a first pass, from the top down:

1. [`table.rs`](../crates/mahjong-server/src/table.rs) — `Table` holds the whole
   game: scores, the dealer, the round wind, the honba counter, the riichi
   deposits, and `GameSettings` (starting score, East-only or hanchan, and the
   `Settings` rule toggles — including three-player). It creates each `Round`,
   decides whether the dealer repeats, and decides when the game is over.
2. [`round/mod.rs`](../crates/mahjong-server/src/round/mod.rs) — one hand. Holds
   the `Round`'s players, the wall, the `TurnPhase` state machine
   (`Draw` → `WaitForDiscard` → `WaitForCalls` → …), and the event queue. The
   flow lives in siblings: [`turn.rs`](../crates/mahjong-server/src/round/turn.rs)
   (draws, discards, kans, North extraction),
   [`calls.rs`](../crates/mahjong-server/src/round/calls.rs) (call detection and
   priority resolution), [`win.rs`](../crates/mahjong-server/src/round/win.rs)
   (riichi and tsumo), [`draws.rs`](../crates/mahjong-server/src/round/draws.rs)
   (exhaustive and abortive draws), and
   [`diagnostics.rs`](../crates/mahjong-server/src/round/diagnostics.rs)
   (opt-in logging, `MAHJONG_ROUND_DIAGNOSTICS=1`).
3. [`legality.rs`](../crates/mahjong-server/src/legality.rs) — which moves a
   player may make, stated against a `Player` plus a `TableContext` rather than
   against a live `Round`, so a client reconstructing its own state can ask the
   same questions the server does.
4. [`scoring.rs`](../crates/mahjong-server/src/scoring.rs) — judges a win from a
   player's hand and hand state, then computes the point transfers. Thin over
   `mahjong-core`; the actual han/fu/score arithmetic is there.
5. [`wall.rs`](../crates/mahjong-server/src/wall.rs) and
   [`player.rs`](../crates/mahjong-server/src/player.rs) — the tile wall
   (including the dead wall, dora indicators, and the three-player 108-tile set)
   and per-player state.

Determinism: `start_game_with_seed` seeds the whole game, not just the first
hand, so a full game replays identically from a seed. Tests and the CPU
simulation rely on this.

## Inside `mahjong-core`

- [`tile.rs`](../crates/mahjong-core/src/tile.rs) — `Tile` and `Wind`. 34 kinds,
  red fives distinguished.
- [`hand.rs`](../crates/mahjong-core/src/hand.rs) — a hand plus its drawn tile
  and melds.
- [`hand_info/hand_analyzer.rs`](../crates/mahjong-core/src/hand_info/hand_analyzer.rs)
  — the analyzer: decomposes a hand into blocks and computes the shanten number
  (`calc_shanten_number`), including the seven-pairs and thirteen-orphans forms.
  This is the hottest code in the project; the CPU calls it constantly.
- [`winning_hand/checker.rs`](../crates/mahjong-core/src/winning_hand/checker.rs)
  — the yaku evaluation entry point. The individual checks are filed by han
  value: `check_1_han.rs`, `check_2_han.rs`, `check_3_han.rs`, `check_5_han.rs`,
  `check_6_han.rs`, `check_yakuman.rs`. Adding or fixing a yaku means editing the
  file for its han value and registering it in the checker.
- [`winning_hand/name.rs`](../crates/mahjong-core/src/winning_hand/name.rs) —
  display names per language. The English names are the WRC ones from the
  glossary.
- [`scoring/fu.rs`](../crates/mahjong-core/src/scoring/fu.rs) and
  [`scoring/score.rs`](../crates/mahjong-core/src/scoring/score.rs) — minipoints,
  base points, rank (mangan and above), and the rounding rules.
- [`settings.rs`](../crates/mahjong-core/src/settings.rs) — rule settings and
  `Lang`, shared by every crate.

## The CPU opponent

Under [`cpu/`](../crates/mahjong-server/src/cpu). A CPU is configured with a
level (weak / normal / strong) and a personality (balanced / speedy /
high-value / defensive), and it plays through the protocol like anyone else.

| File | Role |
|---|---|
| [`client.rs`](../crates/mahjong-server/src/cpu/client.rs) | The seat itself: takes `ServerEvent`s, returns `ClientAction`s. `CpuConfig`, `CpuLevel`, `CpuPersonality` live here. |
| [`state.rs`](../crates/mahjong-server/src/cpu/state.rs) | `CpuGameState`, rebuilt from the event stream. It holds only what a human at that seat would know — this is the crate's honesty guarantee. |
| [`evaluator.rs`](../crates/mahjong-server/src/cpu/evaluator.rs) | For each possible discard: resulting shanten, tile acceptance, and estimated hand value. |
| [`heuristics.rs`](../crates/mahjong-server/src/cpu/heuristics.rs) | Human-style discard wisdom as score adjustments on candidates. Each heuristic is a `DiscardHeuristic` registered in `DISCARD_HEURISTICS`, and only the ones enabled for the CPU's level apply — that registry, rather than a pile of branches, is what makes per-level toggling and per-heuristic testing possible. |
| [`defense.rs`](../crates/mahjong-server/src/cpu/defense.rs) | Tile safety (genbutsu, suji, walls, honours, terminals) combined with threat models per opponent: riichi, melds, flush signs, yakuman signs. |
| [`personalities.rs`](../crates/mahjong-server/src/cpu/personalities.rs) | The parameter set behind each personality. |

To change how the CPU plays, you almost always want a new `DiscardHeuristic` in
`heuristics.rs` rather than a branch elsewhere. Validate with the simulation
example (see [CONTRIBUTING.md](../CONTRIBUTING.md#cpu-simulations)) and quote the
numbers.

## The client

[`main.rs`](../crates/mahjong-client/src/main.rs) runs a macroquad loop: poll
adapter events, feed them to the game state, render, handle input, repeat.

- [`adapter/`](../crates/mahjong-client/src/adapter) — the boundary. The
  `GameAdapter` trait is a handful of calls — `send_action`, `poll_events`,
  `tick`, `request_next_round`, `is_game_over` — implemented twice:
  `LocalAdapter` wraps a `GameDriver` in-process, `RemoteAdapter` speaks
  WebSocket to `mahjong-net-server`. The rest of the UI does not know which one
  it has.
- [`game/`](../crates/mahjong-client/src/game) — client-side state driven by
  events (`events.rs`), input handling (`input.rs`), and setup/lobby state
  (`setup.rs`). Events are queued and applied on a timer rather than
  immediately: a declaration (call, riichi, pei, win) shows its banner and holds
  the following events back, so the player sees the call-out before its effect.
  What is on screen therefore lags the protocol slightly, on purpose.
- [`renderer/`](../crates/mahjong-client/src/renderer) — drawing and hit testing
  together, immediate-mode: `menu.rs` (title, mode, and rule screens),
  `board.rs`, `tiles.rs`, `overlay.rs` (call and win overlays), `result.rs`,
  `banners.rs` (declaration bubbles), `online.rs`, `theme.rs`, `labels.rs`
  (the English-mode tile index labels, baked into the textures rather than drawn
  each frame).
- [`i18n/mod.rs`](../crates/mahjong-client/src/i18n/mod.rs) — every UI string,
  with all languages required per key.
- [`transport.rs`](../crates/mahjong-client/src/transport.rs) — a non-blocking
  WebSocket pollable once per frame. Native uses `tungstenite`; WASM uses
  hand-written JavaScript glue.
- [`wasm_rng.rs`](../crates/mahjong-client/src/wasm_rng.rs) and
  [`persistence.rs`](../crates/mahjong-client/src/persistence.rs) — the two
  places where the native and WASM targets genuinely differ (randomness and
  saved settings).

## The online server

[`mahjong-net-server`](../crates/mahjong-net-server) is server-authoritative and
holds no game rules of its own: it owns a `GameDriver` per room and relays.

- [`connection.rs`](../crates/mahjong-net-server/src/connection.rs) — one
  WebSocket connection: the Hello/Welcome handshake, lobby operations, then
  message relay. Two tasks per connection (read and write).
- [`lobby.rs`](../crates/mahjong-net-server/src/lobby.rs) — room code to room
  channel. A lock held only across create/lookup/remove; no game state.
- [`room.rs`](../crates/mahjong-net-server/src/room.rs) — one room is one tokio
  task owning the driver, processing `RoomMsg`s over an mpsc channel. Seats
  vacated by disconnects are taken over by a CPU, and a returning player
  resyncs from their buffered events.
- [`peers.rs`](../crates/mahjong-net-server/src/peers.rs) — multi-machine
  support. Rooms live in one machine's memory, so a machine that does not own a
  room looks it up on its peers and answers with a `fly-replay` header.
- [`ratelimit.rs`](../crates/mahjong-net-server/src/ratelimit.rs) — per-IP
  room-entry limits against room-code brute forcing.
- [`protocol/net.rs`](../crates/mahjong-server/src/protocol/net.rs) (in
  `mahjong-server`) — the JSON envelopes. In-game traffic wraps `ClientAction`
  and `ServerEvent` unchanged, which is why the wire format follows the seam
  automatically.

## mjai support

[`mahjong-mjai`](../crates/mahjong-mjai) reduces to a bidirectional codec
between mjai events and `ServerEvent` / `ClientAction`:

- [`encode.rs`](../crates/mahjong-mjai/src/encode.rs) — `ServerEvent` → mjai, for
  one seat, concealing what that seat cannot see.
- [`decode.rs`](../crates/mahjong-mjai/src/decode.rs) — mjai → `ServerEvent`, so
  our CPU can be dropped into an mjai host.
- [`bot.rs`](../crates/mahjong-mjai/src/bot.rs) and
  [`host.rs`](../crates/mahjong-mjai/src/host.rs) — the two directions wired up:
  our CPU as someone else's bot, and someone else's bot at our table.
- [`record.rs`](../crates/mahjong-mjai/src/record.rs) — replay mode: all four
  seats collected into a fully revealed log for review tools.

## Big files

Some files are large. Knowing which before you open them saves surprise:

| File | Lines | Note |
|---|---|---|
| [`round/tests.rs`](../crates/mahjong-server/src/round/tests.rs) | ~2,240 | Hand-flow tests. Mostly independent cases; skim for the one you need. |
| [`cpu/heuristics_tests.rs`](../crates/mahjong-server/src/cpu/heuristics_tests.rs) | ~2,130 | One block per heuristic. |
| [`client/game/tests.rs`](../crates/mahjong-client/src/game/tests.rs) | ~1,920 | Client state tests. |
| [`cpu/client_tests.rs`](../crates/mahjong-server/src/cpu/client_tests.rs) | ~1,840 | CPU decision tests. |
| [`adapter/remote.rs`](../crates/mahjong-client/src/adapter/remote.rs) | ~1,700 | Connection, join, resync, and reconnect flow. The trickiest file in the client. |
| [`cpu/heuristics.rs`](../crates/mahjong-server/src/cpu/heuristics.rs) | ~1,560 | The heuristic registry; entries are independent. |
| [`server/scoring.rs`](../crates/mahjong-server/src/scoring.rs) | ~1,500 | Win judgement and payments. |
| [`table.rs`](../crates/mahjong-server/src/table.rs) | ~1,480 | Whole-game state. |
| [`hand_info/hand_analyzer.rs`](../crates/mahjong-core/src/hand_info/hand_analyzer.rs) | ~1,480 | Shanten. Dense, and the place to be most careful. |
| [`driver.rs`](../crates/mahjong-server/src/driver.rs) | ~1,410 | The event pump. |

Outside `mahjong-core`, every module carries a `//!` header explaining what it is
for (`mahjong-core` documents its modules from `lib.rs` instead); reading those
first is usually faster than reading the code.
