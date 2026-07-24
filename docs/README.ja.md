# Riichi Mahjong RS

[![Build and test](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml/badge.svg?branch=main)](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml)

![ロゴ](./img/logo_small.png)

[English version](../README.md)

麻雀（一般的なリーチ麻雀）のRustでの実装です。
**[こちら](https://riichi-mahjong-rs.vercel.app) でプレイすることができます。**

## スクリーンショット

| ![プレイ中の画面1](./img/screenshots/ja/image1.png) | ![プレイ中の画面2](./img/screenshots/ja/image2.png) |
|---|---|
| ![細かいルール設定が可能](./img/screenshots/ja/image3.png) | ![CPUも設定可能](./img/screenshots/ja/image4.png) |

## 特徴

- ネイティブのデスクトップアプリと WASM を利用したブラウザ版の両方で遊べる麻雀ゲーム（日本で一般的なリーチ麻雀ルール）
  - ネイティブ版では `F12` キーで `screenshots/` に PNG スクリーンショットを保存可能
- クライアント UI の多言語表示に対応（現在は日本語と英語）
  - 英語表示では、漢字が読めなくても牌を判別できるよう、牌の右上に小さなラベルを表示する（数牌はスート別の色付き数字〈萬子=赤・筒子=青・索子=緑〉、風牌は E/S/W/N、三元牌は P/F/C）
- 強さ（弱 / 普通 / 強）と性格（バランス / スピード / 高打点 / 守備型）を選択できる CPU 対戦
  - 牌効率・鳴き・リーチ/ダマ判断・押し引き・脅威ベースの守備（スジ/染め手/役満読みを含む）といった定石に基づいて打牌する
- `mahjong-net-server` によるルームコード制のオンライン対戦
  - ホストがルームを作成し 6 文字のコードを共有、友人が参加。空席は CPU が埋める
  - 切断したプレイヤーは CPU が代打ちし、再入室で状態を再同期できる
- 四人麻雀と三人麻雀（三麻）に、ローカル・オンラインの両方で対応
  - 萬子 2〜8 を除いた 108 枚（赤ドラは 5p/5s の 2 枚）、チーなし。ツモ損の有無を対局ごとに切替可能
  - 北抜き（北を晒して抜きドラ 1 翻 + 補充ツモ）に対応。対局ごとに有無を切替可能。萬子のドラは 1m ↔ 9m でループする
  - 35,000 点持ち。東風戦は東 1〜3 局、半荘戦は東 1〜南 3 局
- 四人麻雀・三人麻雀ともに、東風戦と半荘戦を対局ごとに選択可能
- 細かなルールを対局ごとに柔軟に設定可能。例：喰いタン、複数ロンの可否（頭ハネ）、喰い替え、各種途中流局、役満の包、ダブル役満、三人麻雀の北抜きやツモ損
- 同梱のスクリプトと設定により、静的 Web クライアントを Vercel または itch.io、オンライン対戦サーバを Fly.io にデプロイ可能

## 構成

### クレート構成

現在このリポジトリは次のクレートから構成されています。

- `mahjong-core`: 手牌表現やシャンテン数計算、役判定、符計算、点数計算などのコアロジック
- `mahjong-server`: ローカル対局で使う進行管理やルール処理
- `mahjong-client`: ネイティブ実行とブラウザ実行の両方に対応した、Macroquad ベースの 4 人打ち麻雀クライアント
- `mahjong-net-server`: オンラインのルームコード対戦をホストする単一バイナリの WebSocket サーバ（tokio + axum）

### ディレクトリ構成

- `crates/`: ワークスペースの各クレート
- `assets/`: フォントなどの実行時アセット
- `assets/web/`: ブラウザ版クライアントのソース HTML と favicon
- `public/`: ローカル配信・デプロイ用に生成される Web アセット
- `scripts/`: デプロイで使うビルドスクリプト
- `vercel.json`: Vercel のビルド設定

## 開発

最初に、Rust コンパイラと Cargo の最新安定版がインストールされていることを確認します。

~~~sh
rustc --version
cargo --version
~~~

Rust または Cargo が未導入の場合は[rustup](https://rustup.rs) を使ってインストールし、各プラットフォーム向けの案内に従ってセットアップしてください。

その後、リポジトリを clone して、プロジェクトディレクトリへ移動します。

~~~sh
git clone git@github.com:h1g0/riichi_mahjong_rs.git
cd riichi_mahjong_rs
~~~

ローカルでWASMでの実行を行いたい場合、WASMターゲットを追加してください。

~~~sh
rustup target add wasm32-unknown-unknown
~~~

ブラウザ向けビルドスクリプトの実行には、標準的な Unix
ユーティリティを含む Bash 環境と Python 3 も必要です。

### コマンド

テストの実行:

~~~sh
cargo test
~~~

ネイティブ版クライアントのローカル実行:

~~~sh
cargo run -p mahjong-client
~~~

ブラウザ向けクライアントをローカルビルド:

~~~sh
bash scripts/vercel-build.sh
~~~

このスクリプトは WASM クライアントをビルドし、`Cargo.lock` で選択された
Macroquad パッケージから `mq_js_bundle.js` をコピーして、生成済みの Web
アプリケーションを `public/` 配下に配置します。ブラウザで確認する際は
このディレクトリを静的ファイルサーバーで配信します。

例：

npxがインストールされている場合：

~~~sh
npx serve public
~~~

Pythonがインストールされている場合：

~~~sh
python -m http.server 8080 --directory public
~~~

## コントリビューション

このプロジェクトへのコントリビューションを歓迎します。コントリビューションの際は、リーチ麻雀の用語を理解し、コード、コメント、ドキュメントで一貫した用語を使用するために、[ユビキタス言語集](./glossary.ja.md)を参照してください。[英語版](./glossary.md)も利用できます。

## Vercel デプロイ

このプロジェクトは、デプロイのたびに生成済み WASM をコミットしなくても、Vercel 上でビルドできるようになっています。

1. リポジトリを Vercel にインポートします。
2. プロジェクトルートはこのリポジトリのルートのままにします。
3. デプロイすると、vercel.json に従って次のコマンドが実行されます。

~~~sh
bash scripts/vercel-install.sh
bash scripts/vercel-build.sh
~~~

Vercel のビルドでは次の処理を行います。

- 必要に応じて `rustup` を導入
- `wasm32-unknown-unknown` ターゲットを追加
- `mahjong-client` を release ビルド
- Cargo で解決された Macroquad パッケージから JavaScript bundle をコピー
- `assets/web/` からソース HTML と favicon をコピー
- デプロイ用の Web アセットを `public/` 配下に配置

同じ流れをローカルで再現する場合は、Bash、curl、Rust、および WASM ターゲットが利用できる環境で同等の手順を実行してください。

`Cargo.lock` 内の Macroquad を更新すると、次回のブラウザ向けビルドから、
そのパッケージ版に同梱された bundle が自動的に使用されます。

デプロイした Web クライアントをオンラインサーバへ接続させるには、Vercel プロジェクトの環境変数 `MAHJONG_SERVER_URL` を設定します（例: `wss://your-app.fly.dev/ws`）。ビルド時に `window.MAHJONG_SERVER_URL` へ注入されます。未設定の場合は `ws://127.0.0.1:8080/ws`（ローカル開発用）にフォールバックします。

## itch.io デプロイ

GitHub Actions のワークフローは、生成した `public/` ディレクトリを
[butler](https://itch.io/docs/butler/) で itch.io へアップロードできます。
`main` への push ですべてのチェックに成功したときだけデプロイし、Pull
Request では実行しません。必要な設定がない場合、デプロイ処理はスキップします。

初回のみ、次の設定が必要です。

1. itch.io でプロジェクトページを作成し、ゲームの種類を HTML にします。
2. GitHub Actions の Repository secret `BUTLER_API_KEY` を追加します。
   `butler login` または itch.io の API keys 設定画面で取得し、ログには
   出力しないでください。
3. Actions の Repository variable `ITCH_TARGET` を
   `作者名/ゲームのURLスラッグ` 形式で追加します。たとえばページが
   `https://creator.itch.io/riichi-mahjong-rs` なら
   `creator/riichi-mahjong-rs` です。
4. Actions の Repository variable `MAHJONG_SERVER_URL` に本番の
   WebSocket エンドポイント（例: `wss://your-app.fly.dev/ws`）を設定します。
   オンライン対戦にはこの設定が必要です。未設定の場合、itch.io 版は
   ローカル開発用の接続先へフォールバックします。
5. 最初のワークフロー実行で `html5` チャンネルが作られた後、itch.io の
   プロジェクト編集画面で、そのアップロードを HTML5 / Playable in browser
   に指定し、埋め込み設定を行って保存します。

ワークフローは `public/` を直接 `butler push` に渡すため、CI で ZIP を作る
必要はありません。以後、`main` への push が成功するたびに同じ `html5`
チャンネルを更新し、Web ビルドの内容が同一の場合はアップロードを省略します。

## オンライン対戦サーバ

`mahjong-net-server` はルームコード制のオンライン対戦をホストします。静的 Web クライアントとゲームサーバは別々にデプロイします（Vercel は静的配信のみのため、WebSocket サーバは別ホストが必要）。

### ローカルで動かす

~~~sh
cargo run -p mahjong-net-server
~~~

環境変数:

- `PORT`: リッスンポート（デフォルト `8080`）
- `RUST_LOG`: ログフィルタ（例: `mahjong_net_server=debug`）
- `ALLOWED_ORIGINS`: WebSocket 接続を許可する完全一致の `Origin` をカンマ区切りで指定（例: `https://your-app.vercel.app,https://html-classic.itch.zone`）。どちらの Origin 設定もなければ全許可
- `ALLOWED_ORIGIN`: 従来の単一 Origin 設定。後方互換性のため、設定した値は `ALLOWED_ORIGINS` に追加されます。**どちらかの設定に Origin が含まれる場合、`Origin` ヘッダを送らないネイティブクライアントは弾かれます（HTTP 403）** — ネイティブから接続したい場合は両方とも未設定にし、ブラウザクライアント + 組み込みのレート制限で運用してください
- `INTERNAL_PORT`: 複数マシン構成でルーム所在を照会し合う、マシン間専用リスナーのポート（デフォルト `8081`）。Fly 上ではプライベートネットワーク（6PN）のアドレス（`FLY_PRIVATE_IP`）に、ローカルでは `127.0.0.1` に bind します
- `MAHJONG_PEERS`: ピアの内部リスナーをカンマ区切りの `host:port` で指定し、デフォルトのピア発見（`<FLY_APP_NAME>.internal` DNS）を上書きします。複数マシン構成をローカルで試すときに使います

`GET /healthz` は `ok` を返します（ヘルスチェック用）。WebSocket は `GET /ws`。

ローカルサーバと対戦するには、`MAHJONG_SERVER_URL` を指定してネイティブクライアントを起動します。

~~~sh
MAHJONG_SERVER_URL=ws://127.0.0.1:8080/ws cargo run -p mahjong-client
~~~

### Fly.io へのデプロイ

リポジトリに `Dockerfile` と `fly.toml` を同梱しています。TLS（`wss://`）は Fly のプロキシが終端するため、サーバ自体は `PORT` で平文 WebSocket を待ち受けます。

~~~sh
# 初回: アプリを作成（fly.toml の app 名を変更するか fly launch に任せる）
fly launch --no-deploy

# （任意）接続を許可する Origin を Web クライアントに制限
fly secrets set ALLOWED_ORIGINS=https://your-app.vercel.app,https://html-classic.itch.zone

# デプロイ
fly deploy
~~~

デプロイ後、Vercel の `MAHJONG_SERVER_URL` を `wss://<your-app>.fly.dev/ws` に設定して Web クライアントを再デプロイします。

Docker が動く環境ならコンテナとしてどこでも実行できます。

~~~sh
docker build -t mahjong-net-server .
docker run -e PORT=8080 -p 8080:8080 mahjong-net-server
~~~

### 運用メモ

- **複数マシンで運用できます**。ルームはメモリ上にあり、生成されたマシンに固定されますが、参加・再接続は `/ws?room=CODE` クエリパラメータでルームコードを運びます。ルームを所持しないマシンに着地した接続は、ピア照会（`<app>.internal` DNS と内部リスナー、既定ポート 8081）で所持マシンを特定し、`fly-replay` ヘッダで Fly Proxy がそのマシンへ転送します。`fly scale count 2` 以上でスケールでき、新規接続がいつ追加マシンへ振り分けられる（停止中なら起動する）かは `fly.toml` の `[http_service.concurrency]` で制御します。
- **コールドスタート**。しばらく無接続だと、最初の接続でマシン起動まで数秒待ちます（初回はリトライが必要なことがあります）。常時起動にしたい場合は `auto_stop_machines = "off"` / `min_machines_running = 1` にします（課金は増えます）。
- **ルームは再起動で消えます**。再デプロイ・再起動で進行中のルームは消えます（参加者は新しいルームを作り直して再開）。永続化層はありません。アクティブな接続を持つマシンは自動停止されないため、アイドル停止で消えるのは参加者全員が切断済みのルームだけです。
- **レート制限はマシン単位です**。IP 単位の入室レート制限は各マシンのメモリで管理されるため、N 台構成では実質の上限が最大 N 倍になります（カジュアル用途では許容範囲です）。
- `GET /healthz` を監視します（Fly は 15 秒ごとにチェックする設定）。
- サーバは IP 単位の入室レート制限と接続ごとのメッセージ/フレームサイズ上限を適用します。カジュアル用途では追加の WAF は不要です。
