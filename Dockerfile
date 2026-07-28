# syntax=docker/dockerfile:1@sha256:87999aa3d42bdc6bea60565083ee17e86d1f3339802f543c0d03998580f9cb89

FROM rust:1.97.1-bookworm@sha256:77fac8b98f9f46062bb680b6d25d5bcaabfc400143952ebc572e924bcbedc3fa AS builder

WORKDIR /build

COPY Cargo.toml Cargo.lock build.rs ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --locked --release --bin retsu

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN set -eux; \
    mkdir -p /runtime/etc/ssl/certs /runtime/usr/local/bin; \
    install -m 0555 target/release/retsu /runtime/usr/local/bin/retsu; \
    install -m 0444 /etc/nsswitch.conf /runtime/etc/nsswitch.conf; \
    install -m 0444 /etc/ssl/certs/ca-certificates.crt \
        /runtime/etc/ssl/certs/ca-certificates.crt; \
    ldd target/release/retsu \
        | awk '{ for (field = 1; field <= NF; field++) if ($field ~ /^\//) print $field }' \
        | sort -u \
        | while read -r library; do \
            destination="/runtime$(dirname "$library")"; \
            mkdir -p "$destination"; \
            cp -L "$library" "$destination/"; \
        done; \
    find /runtime -exec touch -d '@0' {} +

FROM scratch AS runtime

LABEL org.opencontainers.image.title="retsu" \
      org.opencontainers.image.description="an observable, distributed priority queue" \
      org.opencontainers.image.source="https://github.com/karanbalani/retsu" \
      org.opencontainers.image.licenses="Apache-2.0"

COPY --from=builder /runtime/ /

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

USER 65532:65532

EXPOSE 2424 24247
STOPSIGNAL SIGTERM

ENTRYPOINT ["/usr/local/bin/retsu"]
CMD ["api"]
