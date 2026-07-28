# syntax=docker/dockerfile:1

FROM rust:1.97.1-bookworm AS builder

WORKDIR /build

COPY Cargo.toml Cargo.lock build.rs rust-toolchain.toml ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --locked --release --bin retsu

FROM debian:bookworm-slim AS runtime

LABEL org.opencontainers.image.title="retsu" \
      org.opencontainers.image.description="An observable, distributed priority queue" \
      org.opencontainers.image.source="https://github.com/karanbalani/retsu" \
      org.opencontainers.image.licenses="Apache-2.0"

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 retsu \
    && useradd --uid 10001 --gid 10001 --no-create-home \
        --home-dir /nonexistent --shell /usr/sbin/nologin retsu

WORKDIR /app

COPY --from=builder /build/target/release/retsu /usr/local/bin/retsu
COPY --chown=10001:10001 config/retsu.yaml ./config/retsu.yaml

USER 10001:10001

EXPOSE 2424 24247
STOPSIGNAL SIGTERM

ENTRYPOINT ["/usr/local/bin/retsu"]
CMD ["api"]
