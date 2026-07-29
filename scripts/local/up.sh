#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

compose=(docker compose --file infra/local/compose.yaml)

"${compose[@]}" up -d --wait postgres dragonfly
"${compose[@]}" exec -T postgres \
    sh /docker-entrypoint-initdb.d/observability.sh
"${compose[@]}" --profile observability up -d --wait
./scripts/local/ready.sh

api_address="$("${compose[@]}" port api 2424 | tail -n 1)"
grafana_address="$(
    "${compose[@]}" --profile observability port grafana 3000 \
        | tail -n 1
)"
echo "API:     http://${api_address}"
echo "Grafana: http://${grafana_address}"
