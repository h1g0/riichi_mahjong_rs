# アーキテクチャ

初めてこのコードベースに触れる人のための地図です。5 つのクレートで約 5.7 万行
ありますが、そのほとんどは 1 つの継ぎ目
— [継ぎ目](#継ぎ目-serverevent--clientaction) で説明するイベントストリーム —
にぶら下がっています。まずその節を読んでください。残りは必要になったときに引く
詳細です。

ビルド・テスト・変更の出し方は
[docs/CONTRIBUTING.ja.md](./CONTRIBUTING.ja.md) を、用語は
[ユビキタス言語集](./glossary.ja.md) を参照してください。

英語版は [architecture.md](architecture.md) に並行して管理しています。

## クレート構成

```
mahjong-core          ルール: 牌、手牌、シャンテン数、役、符、点数計算
   ^
   |
mahjong-server        進行: 牌山、卓、局、合法手判定、CPU
   ^        ^      ^
   |        |      |
client   net-server  mjai
```

| クレート | 担当 | 依存 |
|---|---|---|
| [`mahjong-core`](../crates/mahjong-core) | 純粋なルール。牌の表現、手牌解析、シャンテン数、役判定、符と点数の計算。I/O なし、進行なし、UI なし。 | — |
| [`mahjong-server`](../crates/mahjong-server) | ゲーム進行。牌山、ゲーム全体（東風戦・半荘）を通した卓、1 局ずつの進行、合法手の判定、CPU 対戦相手。同期的で I/O を持ちません。 | core |
| [`mahjong-client`](../crates/mahjong-client) | Macroquad 製 GUI（ネイティブ / WASM）。メニュー、卓の描画、入力、i18n。 | core, server |
| [`mahjong-net-server`](../crates/mahjong-net-server) | オンライン対戦。ルームコード方式のロビーを持つ WebSocket サーバーで、1 ルーム = 1 tokio タスク。 | server |
| [`mahjong-mjai`](../crates/mahjong-mjai) | [mjai](https://mjai.app) プロトコルとの相互変換。本プロジェクトの CPU を既存の麻雀 AI ツールで動かしたり検討にかけたりできます。 | core, server（optional） |

この構成から次の 2 つの原則が導かれます。新規コードへのレビュー指摘の多くは、
結局このどちらかに帰着します。

- **ルールは `mahjong-core` に置く。** server や client にゲームロジックを追加
  する前に、core クレートに既にないかを確認してください。
- **`mahjong-server` は同期的で、ネットワーク I/O もファイル I/O も行わず、時計も
  読みません。** 時刻は `now: f64` 引数として外から注入します。だからこそ同じ
  コードが、macroquad のフレームループでも、非同期サーバーでも、バッチ実行の
  シミュレーションでも、そのまま動きます。

## リポジトリの構成

| パス | 内容 |
|---|---|
| `crates/` | 上記のワークスペースクレート。 |
| `assets/fonts/` | ShipporiMincho-Regular.ttf。実行時に読み込みます（SIL OFL、`ShipporiMincho-OFL.txt` を参照）。 |
| `assets/images/` | 牌と点棒の PNG。クライアントのバイナリに埋め込まれます。 |
| `assets/web/` | ブラウザクライアント用の HTML と favicon のソース。 |
| `crates/mahjong-client/js/` | WASM ビルド用の手書き JavaScript グルーコード。`ws.js`（WebSocket）、`storage.js`（設定の保存）、`loading.js`。 |
| `public/` | Web 向けの生成物。`scripts/vercel-build.sh` が生成するので、手で編集しないでください。 |
| `scripts/` | Vercel のビルド・インストールスクリプトと、そこから呼ばれるアセット処理スクリプト。 |
| `docs/` | 本ドキュメント、用語集、日本語 README、画像。 |
| `Dockerfile`, `fly.toml` | `mahjong-net-server` のコンテナと Fly.io の設定。 |
| `vercel.json` | Web クライアントの Vercel ビルド設定。 |

デプロイ（Web クライアントは Vercel、オンラインサーバーは Fly.io または任意の
Docker ホスト、itch.io は CI から）については
[README](./README.ja.md) を参照してください。

## 継ぎ目: `ServerEvent` / `ClientAction`

`mahjong-server` より上のすべては、
[`protocol/mod.rs`](../crates/mahjong-server/src/protocol/mod.rs) にある 2 つの
enum を通して server と会話します。

- **`ServerEvent`** — その席に伝えられること。`GameStarted`、`TileDrawn`、
  `TileDiscarded`、`CallAvailable`、`PlayerCalled`、`PlayerRiichi`、`RoundWon`、
  `RoundDraw` など。イベントは **席ごと** で、その席が見てよい情報しか含みません
  （`OtherPlayerDrew` は「誰かがツモった」ことだけを伝え、何をツモったかは
  伝えません）。
- **`ClientAction`** — その席が宣言すること。`Discard`、`Riichi`、`Chi`、`Pon`、
  `Kan`、`Pei`、`Tsumo`、`Ron`、`Pass`、`NineTerminals`。

このポンプ役が
[`GameDriver`](../crates/mahjong-server/src/driver.rs) です。`Table` と CPU
クライアントを保持し、実質 2 つの呼び出しを公開しています。

```rust
let events: Vec<ServerEvent> = driver.drain_events(seat);
let accepted: bool = driver.handle_action(seat, action);
```

どちらにも `*_at(.., now: f64)` 形式があり、CPU の「考慮時間」を有効にしている
場合はそちらを使います。

`drain_events` の内側で driver は次を回します。卓に溜まった `(席, イベント)` を
取り出し、人間の席の分はバッファに積み、残りを CPU クライアントに渡し、CPU が
返した行動を適用し、また繰り返す（CPU の行動がさらにイベントを生むため）。
ここで押さえておきたい帰結は、**CPU プレイヤーは特別扱いされていない** という
ことです。CPU も人間と同じ `ServerEvent` を受け取り、同じ `ClientAction` を
返します。CPU に伏せ情報を見せるには明示的に渡すしかなく、そうしている箇所は
ありません。

だからこそ同じ driver が、まったく毛色の違う 3 つのホストを支えられます。

| ホスト | driver の回し方 | ファイル |
|---|---|---|
| ローカル対戦 | macroquad のフレームから `get_time()` 付きで `tick_at` / `drain_events_at` を呼ぶ | [`adapter/local.rs`](../crates/mahjong-client/src/adapter/local.rs) |
| オンライン対戦 | tokio のルームタスクが `run_until_blocked` と `drain_all_events_at` を呼ぶ | [`net-server/src/room.rs`](../crates/mahjong-net-server/src/room.rs) |
| mjai | デコーダーが mjai の JSON から `ServerEvent` 列を組み立てる | [`mjai/src/decode.rs`](../crates/mahjong-mjai/src/decode.rs) |

**機能を追加するときは、ここが出発点です。** 新しい宣言は `ClientAction` の
バリアント、プレイヤーに新しく伝えたい情報は `ServerEvent` のバリアントになり
ます。どちらを足す場合も、それを発行・受理する局のロジック、それで詰まらない
ようにする CPU クライアント、クライアントの描画、そして — オンラインプロトコルは
これらの enum をそのまま JSON にするため — mjai 側で表現できるなら mjai コーデック
にも手が入ります。既存のバリアントにフィールドを足すときは、古いピアがデコード
できるよう `#[serde(default)]` を付けてください。

## 1 局を追いかける

最初に読むなら、上から順にこの順序です。

1. [`table.rs`](../crates/mahjong-server/src/table.rs) — `Table` はゲーム全体を
   保持します。点数、親、場風、本場、供託リーチ棒、そして `GameSettings`
   （持ち点、東風戦か半荘か、三人麻雀を含む `Settings` のルール設定）。各
   `Round` を生成し、連荘するかどうかと、ゲームが終了したかどうかを判断します。
2. [`round/mod.rs`](../crates/mahjong-server/src/round/mod.rs) — 1 局。プレイ
   ヤー、牌山、`TurnPhase` の状態機械（`Draw` → `WaitForDiscard` →
   `WaitForCalls` → …）、イベントキューを持ちます。進行の中身は兄弟モジュールに
   あります。[`turn.rs`](../crates/mahjong-server/src/round/turn.rs)（ツモ、
   打牌、カン、北抜き）、
   [`calls.rs`](../crates/mahjong-server/src/round/calls.rs)（鳴きの検出と優先
   順位の解決）、[`win.rs`](../crates/mahjong-server/src/round/win.rs)（立直と
   ツモ和了）、[`draws.rs`](../crates/mahjong-server/src/round/draws.rs)（荒牌
   平局と途中流局）、
   [`diagnostics.rs`](../crates/mahjong-server/src/round/diagnostics.rs)
   （任意で有効化するログ、`MAHJONG_ROUND_DIAGNOSTICS=1`）。
3. [`legality.rs`](../crates/mahjong-server/src/legality.rs) — そのプレイヤーが
   何をできるか。生きた `Round` ではなく `Player` と `TableContext` に対して
   記述されているので、自分の状態を組み立て直したクライアントからも、サーバーと
   同じ問い合わせができます。
4. [`scoring.rs`](../crates/mahjong-server/src/scoring.rs) — 手牌と局面から和了を
   判定し、点数の授受を計算します。`mahjong-core` の薄いラッパーで、翻・符・
   点数の計算自体は core 側にあります。
5. [`wall.rs`](../crates/mahjong-server/src/wall.rs) と
   [`player.rs`](../crates/mahjong-server/src/player.rs) — 牌山（王牌、ドラ表示牌、
   三人麻雀の 108 枚構成を含む）と、プレイヤーごとの状態。

決定性について: `start_game_with_seed` は最初の 1 局だけでなくゲーム全体に
シードを効かせるので、同じシードからは同じゲームが再現されます。テストと CPU
シミュレーションはこれに依存しています。

## `mahjong-core` の中身

- [`tile.rs`](../crates/mahjong-core/src/tile.rs) — `Tile` と `Wind`。34 種、
  赤ドラは区別されます。
- [`hand.rs`](../crates/mahjong-core/src/hand.rs) — 手牌とツモ牌、副露。
- [`hand_info/hand_analyzer.rs`](../crates/mahjong-core/src/hand_info/hand_analyzer.rs)
  — 解析器。手牌をブロックに分解し、シャンテン数を計算します
  （`calc_shanten_number`）。七対子形・国士無双形も含みます。プロジェクト中で
  最も高頻度に呼ばれるコードで、CPU が絶えず叩いています。
- [`winning_hand/checker.rs`](../crates/mahjong-core/src/winning_hand/checker.rs)
  — 役判定の入口。個々の判定は翻数ごとに分かれています（`check_1_han.rs`、
  `check_2_han.rs`、`check_3_han.rs`、`check_5_han.rs`、`check_6_han.rs`、
  `check_yakuman.rs`）。役の追加や修正は、その翻数のファイルを編集し、checker に
  登録する作業になります。
- [`winning_hand/name.rs`](../crates/mahjong-core/src/winning_hand/name.rs) —
  言語別の表示名。英語名は用語集の WRC 名です。
- [`scoring/fu.rs`](../crates/mahjong-core/src/scoring/fu.rs) と
  [`scoring/score.rs`](../crates/mahjong-core/src/scoring/score.rs) — 符、基本点、
  ランク（満貫以上）、および切り上げの規則。
- [`settings.rs`](../crates/mahjong-core/src/settings.rs) — ルール設定と `Lang`。
  全クレートで共有します。

## CPU 対戦相手

[`cpu/`](../crates/mahjong-server/src/cpu) 以下にあります。CPU はレベル
（weak / normal / strong）と性格（balanced / speedy / high-value / defensive）で
設定され、他のプレイヤーと同じくプロトコル越しに打ちます。

| ファイル | 役割 |
|---|---|
| [`client.rs`](../crates/mahjong-server/src/cpu/client.rs) | 席そのもの。`ServerEvent` を受け取り `ClientAction` を返します。`CpuConfig`、`CpuLevel`、`CpuPersonality` はここ。 |
| [`state.rs`](../crates/mahjong-server/src/cpu/state.rs) | イベント列から組み立て直す `CpuGameState`。その席の人間が知り得る情報しか持ちません。これがこのクレートの「イカサマをしない」保証です。 |
| [`evaluator.rs`](../crates/mahjong-server/src/cpu/evaluator.rs) | 打牌候補ごとの評価。打った後のシャンテン数、受け入れ、推定打点。 |
| [`heuristics.rs`](../crates/mahjong-server/src/cpu/heuristics.rs) | 人間的な打牌のセオリーを、候補への加点・減点として表現します。各セオリーは `DISCARD_HEURISTICS` に登録された `DiscardHeuristic` で、CPU のレベルで有効なものだけが適用されます。分岐の山ではなくこのレジストリ方式にしていることが、レベル別の切り替えとセオリー単位のテストを可能にしています。 |
| [`defense.rs`](../crates/mahjong-server/src/cpu/defense.rs) | 牌の安全度（現物、筋、壁、字牌、端牌）と、相手ごとの脅威モデル（立直、副露、染め手の気配、役満の気配）の組み合わせ。 |
| [`personalities.rs`](../crates/mahjong-server/src/cpu/personalities.rs) | 各性格の背後にあるパラメータ一式。 |

CPU の打ち方を変えたいときは、ほぼ常に、他所に分岐を足すのではなく
`heuristics.rs` に新しい `DiscardHeuristic` を追加するのが正解です。検証には
シミュレーションの example を使い（[コントリビューションガイド](./CONTRIBUTING.ja.md#cpu-シミュレーション)
を参照）、数値を添えてください。

## クライアント

[`main.rs`](../crates/mahjong-client/src/main.rs) が macroquad のループを回し
ます。アダプターからイベントを取得し、ゲーム状態に流し込み、描画し、入力を
処理し、また繰り返す、という流れです。

- [`adapter/`](../crates/mahjong-client/src/adapter) — 境界。`GameAdapter`
  トレイトは数個の呼び出し（`send_action`、`poll_events`、`tick`、
  `request_next_round`、`is_game_over`）で、実装は 2 つあります。`LocalAdapter`
  はプロセス内の `GameDriver` を包み、`RemoteAdapter` は WebSocket で
  `mahjong-net-server` と話します。UI の他の部分は、どちらを掴んでいるかを
  知りません。
- [`game/`](../crates/mahjong-client/src/game) — イベント駆動のクライアント側
  状態（`events.rs`）、入力処理（`input.rs`）、設定・ロビーの状態
  （`setup.rs`）。イベントは即座にではなくタイマーを挟んで適用されます。宣言
  （鳴き、立直、北抜き、和了）はまずバナーを出し、後続のイベントを保留するので、
  プレイヤーには「発声 → その効果」の順で見えます。つまり画面はプロトコルより
  意図的にわずかに遅れます。
- [`renderer/`](../crates/mahjong-client/src/renderer) — 即時モードで描画と当たり
  判定を同時に行います。`menu.rs`（タイトル・モード選択・ルール設定）、
  `board.rs`、`tiles.rs`、`overlay.rs`（鳴き・和了のオーバーレイ）、
  `result.rs`、`banners.rs`（発声の吹き出し）、`online.rs`、`theme.rs`、
  `labels.rs`（英語表示時の牌のインデックス表記。毎フレーム描くのではなく
  テクスチャに焼き込んでいます）。
- [`i18n/mod.rs`](../crates/mahjong-client/src/i18n/mod.rs) — UI 文字列の集約。
  キーごとに全言語の定義が必須です。
- [`transport.rs`](../crates/mahjong-client/src/transport.rs) — 毎フレーム
  ポーリングできるノンブロッキングな WebSocket。ネイティブは `tungstenite`、
  WASM は手書きの JavaScript グルーコードを使います。
- [`wasm_rng.rs`](../crates/mahjong-client/src/wasm_rng.rs) と
  [`persistence.rs`](../crates/mahjong-client/src/persistence.rs) — ネイティブと
  WASM で実装が本当に分かれる 2 箇所（乱数と設定の保存）。

## オンラインサーバー

[`mahjong-net-server`](../crates/mahjong-net-server) はサーバー権威型で、独自の
ゲームルールは一切持ちません。ルームごとに `GameDriver` を保持して中継するだけ
です。

- [`connection.rs`](../crates/mahjong-net-server/src/connection.rs) — 1 本の
  WebSocket 接続。Hello/Welcome のハンドシェイク、ロビー操作、その後のメッセージ
  中継。1 接続につき読み取りと書き込みの 2 タスクが動きます。
- [`lobby.rs`](../crates/mahjong-net-server/src/lobby.rs) — ルームコードから
  ルームのチャネルへの対応表。ロックは作成・検索・削除の間だけ保持し、ゲーム状態
  は持ちません。
- [`room.rs`](../crates/mahjong-net-server/src/room.rs) — 1 ルーム = 1 tokio
  タスク。driver を所有し、mpsc チャネル越しに `RoomMsg` を処理します。切断で
  空いた席は CPU が引き継ぎ、戻ってきたプレイヤーはバッファされたイベントから
  再同期します。
- [`peers.rs`](../crates/mahjong-net-server/src/peers.rs) — 複数マシン対応。
  ルームは作成したマシンのメモリ上にしか無いため、自分が持たないルームコードが
  来たマシンはピアに問い合わせ、`fly-replay` ヘッダーで転送させます。
- [`ratelimit.rs`](../crates/mahjong-net-server/src/ratelimit.rs) — IP ごとの
  入室レート制限。ルームコードの総当たり対策です。
- [`protocol/net.rs`](../crates/mahjong-server/src/protocol/net.rs)（`mahjong-server`
  側） — JSON のエンベロープ。対局中の通信は `ClientAction` と `ServerEvent` を
  そのまま包むだけなので、ワイヤーフォーマットは継ぎ目に自動的に追従します。

## mjai 対応

[`mahjong-mjai`](../crates/mahjong-mjai) は、mjai イベントと
`ServerEvent` / `ClientAction` の双方向コーデックに還元されます。

- [`encode.rs`](../crates/mahjong-mjai/src/encode.rs) — `ServerEvent` → mjai。
  1 席分を、その席から見えない情報は伏せたまま変換します。
- [`decode.rs`](../crates/mahjong-mjai/src/decode.rs) — mjai → `ServerEvent`。
  本プロジェクトの CPU をそのまま mjai ホストに座らせられます。
- [`bot.rs`](../crates/mahjong-mjai/src/bot.rs) と
  [`host.rs`](../crates/mahjong-mjai/src/host.rs) — 上記 2 方向を組み上げたもの。
  「自分たちの CPU を他所のボットとして動かす」側と、「他所のボットを自分たちの
  卓に座らせる」側です。
- [`record.rs`](../crates/mahjong-mjai/src/record.rs) — リプレイモード。4 席分を
  まとめ、すべて開示した検討ツール向けのログを出力します。

## 大きいファイル

いくつかのファイルはかなり大きいので、開く前に知っておくと驚かずに済みます。

| ファイル | 行数 | 備考 |
|---|---|---|
| [`round/tests.rs`](../crates/mahjong-server/src/round/tests.rs) | 約 2,240 | 局進行のテスト。ほぼ独立したケースの集まりなので、必要な 1 件を拾い読みすれば十分です。 |
| [`cpu/heuristics_tests.rs`](../crates/mahjong-server/src/cpu/heuristics_tests.rs) | 約 2,130 | セオリーごとに 1 ブロック。 |
| [`client/game/tests.rs`](../crates/mahjong-client/src/game/tests.rs) | 約 1,920 | クライアント状態のテスト。 |
| [`cpu/client_tests.rs`](../crates/mahjong-server/src/cpu/client_tests.rs) | 約 1,840 | CPU の判断のテスト。 |
| [`adapter/remote.rs`](../crates/mahjong-client/src/adapter/remote.rs) | 約 1,700 | 接続・参加・再同期・再接続の流れ。クライアントで最も込み入ったファイルです。 |
| [`cpu/heuristics.rs`](../crates/mahjong-server/src/cpu/heuristics.rs) | 約 1,560 | セオリーのレジストリ。各エントリは独立しています。 |
| [`server/scoring.rs`](../crates/mahjong-server/src/scoring.rs) | 約 1,500 | 和了判定と点数の授受。 |
| [`table.rs`](../crates/mahjong-server/src/table.rs) | 約 1,480 | ゲーム全体の状態。 |
| [`hand_info/hand_analyzer.rs`](../crates/mahjong-core/src/hand_info/hand_analyzer.rs) | 約 1,480 | シャンテン数。密度が高く、最も慎重さを要する場所です。 |
| [`driver.rs`](../crates/mahjong-server/src/driver.rs) | 約 1,410 | イベントポンプ。 |

`mahjong-core` を除く各モジュールには、その役割を説明する `//!` ヘッダーが付いて
います（`mahjong-core` は `lib.rs` 側でモジュールを説明しています）。コード本体を
読むより、まずそこを読むほうがたいてい速いです。
