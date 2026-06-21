# riichi_mahjong_rs

[English version](../README.md)

[![Build and test](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml/badge.svg?branch=main)](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml)

麻雀（一般的なリーチ麻雀）のRustでの実装です。

![プレイ中の画面](./image1.png)

## 現在の実装状況

- シャンテン数計算を実装済み
- [役](https://ja.wikipedia.org/wiki/%E9%BA%BB%E9%9B%80%E3%81%AE%E5%BD%B9%E4%B8%80%E8%A6%A7) 判定を実装済み
- 符計算および点数計算を実装済み
- ネイティブ版と WASM 版の両方で動かせるプレイ可能なクライアントを同梱
  - 現在のクライアントは仮の簡易版
  - CPU 対戦を実装済み。強さ（弱 / 普通 / 強）と性格（バランス / スピード / 高打点 / 守備型）を選択可能。牌効率・鳴き・リーチ/ダマ判断・押し引き・脅威ベースの守備（スジ/染め手/役満読みを含む）といった定石に基づいて打牌する
- `mahjong-net-server` によるオンライン対戦（ルームコード制）に対応
  - ホストがルームを作成し 6 文字のコードを共有、友人が参加。空席は CPU が埋める
  - 切断したプレイヤーは CPU が代打ちし、再入室で状態を再同期できる
- 同梱スクリプトを使った Vercel デプロイに対応（静的 Web クライアント）

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
- `public/`: デプロイ用の Web アセット
- `scripts/`: デプロイで使うビルドスクリプト
- `index.html`: WASM クライアント用のローカル Web エントリポイント
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
cargo build -p mahjong-client --target wasm32-unknown-unknown --release
~~~

ビルド後は、お好みの方法でこのリポジトリをローカルの静的ファイルサーバーで配信し、index.html を開くと生成した WASM クライアントをブラウザで確認できます。

例：

npxがインストールされている場合：

~~~sh
npx serve . 
~~~

Pythonがインストールされている場合：

~~~sh
python -m http.server 8080
~~~

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
- デプロイ用の Web アセットを `public/` 配下に配置

同じ流れをローカルで再現する場合は、Bash、curl、Rust、および WASM ターゲットが利用できる環境で同等の手順を実行してください。

デプロイした Web クライアントをオンラインサーバへ接続させるには、Vercel プロジェクトの環境変数 `MAHJONG_SERVER_URL` を設定します（例: `wss://your-app.fly.dev/ws`）。ビルド時に `window.MAHJONG_SERVER_URL` へ注入されます。未設定の場合は `ws://127.0.0.1:8080/ws`（ローカル開発用）にフォールバックします。

## オンライン対戦サーバ

`mahjong-net-server` はルームコード制のオンライン対戦をホストします。静的 Web クライアントとゲームサーバは別々にデプロイします（Vercel は静的配信のみのため、WebSocket サーバは別ホストが必要）。

### ローカルで動かす

~~~sh
cargo run -p mahjong-net-server
~~~

環境変数:

- `PORT`: リッスンポート（デフォルト `8080`）
- `RUST_LOG`: ログフィルタ（例: `mahjong_net_server=debug`）
- `ALLOWED_ORIGIN`: 設定すると、`Origin` ヘッダが一致する WebSocket 接続のみ許可（例: `https://your-app.vercel.app`）。未設定なら全許可。**ネイティブクライアントは `Origin` ヘッダを送らないため、設定中は弾かれます（HTTP 403）** — ネイティブから接続したい場合は未設定にし、ブラウザクライアント + 組み込みのレート制限で運用してください

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
fly secrets set ALLOWED_ORIGIN=https://your-app.vercel.app

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

- **必ず単一マシンで運用すること**。ルームはメモリ上にあり、マシン間で共有**されません**。初回デプロイ後に一度だけ `fly scale count 1 -a <app>` を実行してマシン数を 1 にします。1台であれば `fly.toml` のとおり `auto_stop_machines = "stop"` で問題なく、かつ安価です（無接続時に停止し、次の接続で**同じ1台**がコールドスタートする）。複数マシンには絶対にスケールしないこと（再接続が別マシンに着地してルームが見つからず接続が切れます）。
- **コールドスタート**。しばらく無接続だと、最初の接続でマシン起動まで数秒待ちます（初回はリトライが必要なことがあります）。常時起動にしたい場合は `auto_stop_machines = "off"` / `min_machines_running = 1` にします（課金は増えます）。
- **ルームは再起動で消えます**。再デプロイ・再起動・アイドル停止で進行中のルームは消えます（参加者は新しいルームを作り直して再開）。永続化層はありません。
- `GET /healthz` を監視します（Fly は 15 秒ごとにチェックする設定）。
- サーバは IP 単位の入室レート制限と接続ごとのメッセージ/フレームサイズ上限を適用します。カジュアル用途では追加の WAF は不要です。
