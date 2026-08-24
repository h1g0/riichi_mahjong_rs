# コントリビューションガイド

[English version](../CONTRIBUTING.md)

このプロジェクトに興味を持っていただきありがとうございます。本プロジェクトは
Rust による日本リーチ麻雀の実装です。ルールエンジン、CPU 対戦相手、ネイティブと
ブラウザの両方で動作する Macroquad 製クライアント、そしてオンライン対戦用の
WebSocket サーバーで構成されています。

バグ報告、ルールの誤りの指摘、CPU の打ち筋の改善、翻訳、ドキュメントの修正、
いずれも歓迎します。このドキュメントでは、ビルド・テストの方法と変更の出し方を
説明します。コードベースの全体像は
[docs/architecture.ja.md](./architecture.ja.md) を参照してください。

## 目次

- [環境の準備](#環境の準備)
- [ビルド・実行・テスト](#ビルド実行テスト)
- [コミット前に](#コミット前に)
- [ブランチ・コミット・プルリクエスト](#ブランチコミットプルリクエスト)
- [コーディング規約](#コーディング規約)
- [Good first issue](#good-first-issue)
- [質問する場所](#質問する場所)

## 環境の準備

最新の stable な Rust ツールチェインが必要です。未インストールの場合は
[rustup](https://rustup.rs) から導入してください。

```sh
rustc --version
cargo --version
```

リポジトリを取得します。

```sh
git clone https://github.com/h1g0/riichi_mahjong_rs.git
cd riichi_mahjong_rs
```

ネイティブクライアントと各サーバーはこれだけで動きます。ブラウザ（WASM）版の
ビルドにはさらに WASM ターゲットと、標準的な Unix コマンドが揃った Bash、および
Python 3 が必要です。

```sh
rustup target add wasm32-unknown-unknown
```

## ビルド・実行・テスト

### テスト

```sh
cargo test
```

作業は 1 つのクレートに閉じることが多く、対象を絞ったほうがはるかに高速です。

```sh
cargo test -p mahjong-core
```

```sh
cargo test -p mahjong-server
```

CI では `cargo test --workspace --all-targets --all-features --locked` を実行して
います。ローカルでデフォルトのフラグのまま通っていても、example や結合テストを
壊していると CI で落ちる点に注意してください。

### ネイティブクライアント

```sh
cargo run -p mahjong-client
```

`F12` を押すと `screenshots/` に PNG のスクリーンショットが保存されます。

### ブラウザクライアント

```sh
bash scripts/vercel-build.sh
```

```sh
python -m http.server 8080 --directory public
```

ブラウザで <http://127.0.0.1:8080> を開きます。

このスクリプトは `mahjong-client` を `wasm32-unknown-unknown` 向けにビルドし、
`Cargo.lock` が解決した Macroquad のバージョンから `mq_js_bundle.js` をコピーし、
一式を `public/` に組み立てます。`public/` は生成物なので直接編集せず、
`assets/web/` 側を編集してください。

WASM まわりには意図的な制約が 2 つあり、いずれも気づかずに壊しやすい箇所です。

- `.cargo/config.toml` で `getrandom_backend="custom"` を指定しています。Miniquad
  の WASM ローダーが wasm-bindgen を使わないためで、バックエンドの実装は
  `crates/mahjong-client/src/wasm_rng.rs` にあります。
- **wasm-bindgen への依存を追加しないでください。** ブラウザ API へのアクセスは、
  Web アセットに同梱している手書きの JavaScript グルーコード経由で行っています。

### オンラインサーバー

```sh
cargo run -p mahjong-net-server
```

`PORT`（デフォルト `8080`）で待ち受け、WebSocket のエンドポイントは `/ws`、
ヘルスチェックは `GET /healthz` です。ネイティブクライアントから接続するには
次のようにします。

```sh
MAHJONG_SERVER_URL=ws://127.0.0.1:8080/ws cargo run -p mahjong-client
```

なお `ALLOWED_ORIGINS`（および旧設定の `ALLOWED_ORIGIN`）が設定されていると、
`Origin` ヘッダーを送らないネイティブクライアントは HTTP 403 で拒否されます。
ローカル開発ではどちらも未設定のままにしてください。サーバーの環境変数の一覧は
[README](../README.md#online-multiplayer-server) にあります。

### CPU シミュレーション

CPU AI に手を入れたときは、1 局ではなくまとまった局数で確認してください。

```sh
cargo run -p mahjong-server --release --example cpu_simulation -- 100 42 2>/dev/null
```

引数は「対局数」と「基準シード」です。シードと依存クレートが同じであれば結果は
決定的なので、変更の前後で和了率・放銃率・平均着順を比較すればリグレッション
チェックとして機能します。プルリクエストには変更前後の数値を記載してください。

### mjai ツール

CPU 対戦相手は [mjai](https://mjai.app) のボットとして動かせます。また、対局を
mjai 形式のログとして書き出し、既存の検討ツールにかけることもできます。

```sh
cargo run -p mahjong-mjai --bin mjai-bot -- --level strong --name my-bot
```

```sh
cargo run -p mahjong-mjai --example mjai_export -- 42 > game.mjson
```

## コミット前に

次の 2 つは必ずクリーンにしてください。どちらも CI で失敗します。

```sh
cargo fmt
```

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

加えて、

- **新しい機能にはテストを追加してください。** ユニットテストは対象コードの
  そばに置きます（同一ファイル内の `mod tests`、または隣接する `*_tests.rs`）。
- **バグ修正には必ずリグレッションテストを追加してください。** 何を守るための
  テストなのか次の読み手に伝わるよう、テスト名かコメントに issue 番号を書きます。
- **まず `mahjong-core` を確認してください。** `mahjong-server` や
  `mahjong-client` にゲームロジックを追加する前に、同じものが core クレートに
  すでにないか調べます。シャンテン数、役、符、点数計算は呼び出し側ではなく core
  に置きます。

## ブランチ・コミット・プルリクエスト

ブランチ名は `{type}/{issue-number}-{english-branch-name}` の形式です。

- `fix` — バグ修正
- `feat` — 新機能
- `misc` — その他（ドキュメント、リファクタリング、ツール類）

例: `fix/#87-hide-dora-on-draw-result`

コミットメッセージとプルリクエストの説明は、ドメイン用語の多くが日本語由来で
あっても **英語** で記述してください。「何をしたか」はコードが語るので、
**なぜ** その変更が必要なのかを書きます。

良いプルリクエストとは、

- `main` に向いていて、1 つの issue に絞られている
- 対応する issue にリンクしている
- どう確認したかが書いてある（実行したテスト、手動確認の手順、CPU 変更なら
  シミュレーションの数値）
- UI が変わるならスクリーンショットが付いている

ものです。

## コーディング規約

### 用語

リーチ麻雀の用語は
[ユビキタス言語集](./glossary.ja.md)（[英語版](./glossary.md)）で確定させて
います。英語名は World Riichi Championship (WRC) Rules 2025 に準拠します。
コード・コメント・ドキュメントではこの用語を使い、集に載っていない用語を導入
する場合は用語集も更新してください。

認識の助けになる場合は、括弧書きで日本語を添えます:
`liability payment (pao / 包)`

### コメント

本プロジェクトは次の指針に従います。**How はコード**、**What はテスト**、
**Why はコミットログ**、そして **Why not はコードコメント**。

- 通常のコメント（`//`）には、制約・採用しなかった単純な代替案・回避している
  バグのいずれかを書きます。バグの場合は `#294` のように issue 番号を添えて
  ください。コードを読めば分かることを言い換えただけのコメントは削除します。
- ドキュメンテーションコメント（`///`、`//!`）には API の説明を書きます。目的、
  不変条件、単位、エッジケースなど。短くても残してください。すべてのモジュール
  先頭に `//!` を置くのが本プロジェクトの慣習で、これがコードベースを追える
  ようにしています。
- コメントはすべて英語で書きます。
- **文字列リテラル内の日本語は翻訳・改変しないでください。** UI 文言、i18n の
  文字列、テストデータは「注釈」ではなく「内容」です。

### ユーザーに見える文字列

クライアント UI は多言語対応（日本語・英語）です。固定文言は
`crates/mahjong-client/src/i18n/mod.rs` の `Key` enum に定義します。キーごとに
すべての言語を埋める必要があるため、訳し忘れは暗黙のフォールバックではなく
コンパイルエラーになります。レンダラー側に表示文字列を直接書かないでください。

## Good first issue

[`good first issue`](https://github.com/h1g0/riichi_mahjong_rs/labels/good%20first%20issue)
ラベルの付いた issue は、触るファイルが少なく、ルールエンジン全体を把握して
いなくても取り組める内容です。`documentation` や `help wanted` も見てみて
ください。

該当するラベルの issue が無い場合は、取り組みたい内容を issue として立てて
ください。他の人が着手していないかを確認する意味でも、それが確実です。プル
リクエストを出す前に、対象の issue にコメントして着手を宣言してください。

## 質問する場所

- **バグ・機能要望**:
  [issue](https://github.com/h1g0/riichi_mahjong_rs/issues/new/choose) を立てて
  ください。
- **コードベース、ルールの解釈、実装方針についての質問**: `question` ラベルを
  付けて issue を立てるか、作業中の issue で質問してください。日本語でも英語でも
  構いません。英語で書く必要があるのは、リポジトリに残る成果物（コミット
  メッセージ、プルリクエストの説明、コード中のコメント）だけです。

コントリビューションを行った時点で、その内容が本プロジェクトの
[MIT License](../LICENSE) の下でライセンスされることに同意したものとみなします。
