# riichi_mahjong_rs

[English version](README.md)

[![Build and test](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml/badge.svg?branch=main)](https://github.com/h1g0/mahjong_rs/actions/workflows/build_and_test.yml)

麻雀（一般的なリーチ麻雀）のRustでの実装です。

## 現在の実装状況

- シャンテン数計算を実装済み
- [役](https://ja.wikipedia.org/wiki/%E9%BA%BB%E9%9B%80%E3%81%AE%E5%BD%B9%E4%B8%80%E8%A6%A7) 判定を実装済み
- 符計算および点数計算を実装済み
- ネイティブ版と WASM 版の両方で動かせるプレイ可能なクライアントを同梱
  - 現在のクライアントは仮の簡易版
  - CPU 対戦を実装済み（現在の実装は仮実装）
  - 将来的なネットワーク対戦の実装も考慮した設計
- 同梱スクリプトを使った Vercel デプロイに対応

## 構成

### クレート構成

現在このリポジトリは次のクレートから構成されています。

- `mahjong-core`: 手牌表現やシャンテン数計算、役判定、符計算、点数計算などのコアロジック
- `mahjong-server`: ローカル対局で使う進行管理やルール処理
- `mahjong-client`: ネイティブ実行とブラウザ実行の両方に対応した、Macroquad ベースの 4 人打ち麻雀クライアント

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
