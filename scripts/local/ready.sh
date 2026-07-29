#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

compose=(docker compose --file infra/local/compose.yaml)

wait_for_service() {
    local service="$1"
    local port="$2"
    local path="$3"
    local label="$4"
    local address=""

    for _ in $(seq 1 30); do
        address="$(
            "${compose[@]}" --profile observability \
                port "${service}" "${port}" 2>/dev/null \
                | tail -n 1
        )"
        if [[ -n "${address}" ]] \
            && curl --fail --silent --show-error --max-time 2 \
                "http://${address}${path}" >/dev/null 2>&1; then
            echo "${label} is ready at http://${address}${path}"
            return
        fi
        sleep 2
    done

    echo "${label} did not become ready within 60 seconds." >&2
    "${compose[@]}" --profile observability ps "${service}" >&2
    "${compose[@]}" --profile observability \
        logs --tail 40 "${service}" >&2
    return 1
}

wait_for_service api 2424 /health/ready API
wait_for_service \
    expired-message-cleaner 24247 /health/ready \
    "Expired-message cleaner"
wait_for_service \
    state-metrics-collector 24247 /health/ready \
    "State-metrics collector"
wait_for_service prometheus 9090 /-/ready Prometheus
wait_for_service grafana 3000 /api/health Grafana
