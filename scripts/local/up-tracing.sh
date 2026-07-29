#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

compose=(docker compose --file infra/local/compose.yaml)

"${compose[@]}" --profile tracing up -d --wait otel-collector tempo
RETSU_LOCAL_TRACES_ENABLED=true \
    "${compose[@]}" --profile tracing up -d --no-deps --force-recreate \
    api dead-letter-message-cleaner expired-message-cleaner state-metrics-collector
./scripts/local/ready.sh
