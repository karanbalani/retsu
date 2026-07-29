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

wait_for_prometheus_targets() {
    local address="$1"
    local response=""
    local required_target_query='
        (max(up{job="retsu",component="api"}) == bool 1 or vector(0))
        + (max(up{job="retsu",component="expired-message-cleaner"}) == bool 1 or vector(0))
        + (max(up{job="retsu",component="state-metrics-collector"}) == bool 1 or vector(0))
        + (max(up{job="pgbouncer"}) == bool 1 or vector(0))
        + (max(up{job="postgres"}) == bool 1 or vector(0))
        + (max(up{job="containers"}) == bool 1 or vector(0))
    '

    for _ in $(seq 1 30); do
        response="$(
            curl --fail --silent --show-error --max-time 2 \
                --get \
                --data-urlencode "query=${required_target_query}" \
                "http://${address}/api/v1/query" 2>/dev/null \
                || true
        )"
        if [[ "${response}" == *'"status":"success"'* ]] \
            && [[ "${response}" == *',"6"]}]'* ]]; then
            echo "Required Prometheus scrape targets are ready."
            return
        fi
        sleep 2
    done

    echo "Required Prometheus scrape targets did not become ready within 60 seconds." >&2
    echo "Expected: retsu/api, retsu/expired-message-cleaner, retsu/state-metrics-collector, pgbouncer, postgres, and containers." >&2
    echo "Current target status:" >&2
    curl --fail --silent --show-error --max-time 5 \
        --get \
        --data-urlencode 'query=up{job=~"retsu|pgbouncer|postgres|containers"}' \
        "http://${address}/api/v1/query" >&2 \
        || true
    echo >&2
    return 1
}

wait_for_service api 2424 /health/ready API
wait_for_service \
    expired-message-cleaner 24247 /health/ready \
    "Expired-message cleaner"
wait_for_service \
    dead-letter-message-cleaner 24247 /health/ready \
    "Dead-letter-message cleaner"
wait_for_service \
    state-metrics-collector 24247 /health/ready \
    "State-metrics collector"
wait_for_service prometheus 9090 /-/ready Prometheus
prometheus_address="$(
    "${compose[@]}" --profile observability port prometheus 9090 \
        | tail -n 1
)"
wait_for_prometheus_targets "${prometheus_address}"
wait_for_service grafana 3000 /api/health Grafana
