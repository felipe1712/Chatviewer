# Multi-stage Dockerfile para ChatViewer Pro License Server
FROM rust:latest as builder
WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y pkg-config libssl-dev libsqlite3-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY templates ./templates
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 libsqlite3-0 curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/license-server /app/license-server
COPY --from=builder /usr/src/app/migrations /app/migrations

RUN mkdir -p /app/data

EXPOSE 8095
ENV PORT=8095

CMD ["/app/license-server"]
