# mahjong-net-server（オンライン対戦サーバ）のコンテナイメージ
#
# クライアント（macroquad）はビルドしない。`-p mahjong-net-server` で
# 対象を絞るため、依存グラフ上の mahjong-server / mahjong-core のみ
# コンパイルされる。

# --- ビルドステージ ---
FROM rust:1-slim-bookworm AS builder
WORKDIR /app
# ワークスペース全体をコピーしてサーバだけをリリースビルドする
COPY . .
RUN cargo build --release -p mahjong-net-server

# --- 実行ステージ ---
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/mahjong-net-server /usr/local/bin/mahjong-net-server

# PORT で待ち受ける（ホスティング側が割り当てる）。TLS は前段のプロキシが終端する。
ENV PORT=8080
EXPOSE 8080

CMD ["mahjong-net-server"]
