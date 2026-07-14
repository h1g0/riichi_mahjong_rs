# Container image for the online mahjong-net-server.
#
# The package selection deliberately excludes the macroquad client, so only
# mahjong-net-server and its mahjong-server/mahjong-core dependencies compile.

# Build stage
FROM rust:1-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p mahjong-net-server

# Runtime stage
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/mahjong-net-server /usr/local/bin/mahjong-net-server

# The hosting platform assigns PORT and terminates TLS at its proxy.
ENV PORT=8080
EXPOSE 8080

USER 10001:10001

CMD ["mahjong-net-server"]
