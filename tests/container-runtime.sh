#!/usr/bin/env bash

set -Eeuo pipefail

image="${1:?Usage: tests/container-runtime.sh IMAGE}"
test_id="${GITHUB_RUN_ID:-$$}-${GITHUB_RUN_ATTEMPT:-1}"
network="retsu-integration-${test_id}"
postgres="retsu-postgres-${test_id}"
dragonfly="retsu-dragonfly-${test_id}"
api="retsu-api-${test_id}"
expired_worker="retsu-expired-worker-${test_id}"
metrics_worker="retsu-metrics-worker-${test_id}"
containers=()

database_url="postgres://postgres@${postgres}:5432/postgres?sslmode=disable"
cache_url="redis://${dragonfly}:6379"

cleanup() {
    status=$?
    trap - EXIT

    if (( status != 0 )); then
        for container in "${containers[@]}"; do
            if docker inspect "${container}" >/dev/null 2>&1; then
                docker logs "${container}" >&2 || true
            fi
        done
    fi

    if (( ${#containers[@]} > 0 )); then
        docker rm --force "${containers[@]}" >/dev/null 2>&1 || true
    fi

    docker network rm "${network}" >/dev/null 2>&1 || true
    exit "${status}"
}

wait_for_container_health() {
    local container=$1

    for _ in {1..60}; do
        local status
        status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "${container}")"

        case "${status}" in
            healthy)
                return 0
                ;;
            exited | dead | unhealthy)
                echo "${container} entered ${status} state."
                return 1
                ;;
        esac

        sleep 1
    done

    echo "${container} did not become healthy."
    return 1
}

published_port() {
    local container=$1
    local port=$2

    docker port "${container}" "${port}/tcp" | awk -F: 'NR == 1 { print $NF }'
}

wait_for_http() {
    local container=$1
    local url=$2

    for _ in {1..60}; do
        if curl \
            --connect-timeout 1 \
            --max-time 2 \
            --fail \
            --silent \
            --show-error \
            "${url}" >/dev/null 2>&1; then
            return 0
        fi

        if [[ "$(docker inspect --format '{{.State.Running}}' "${container}")" != "true" ]]; then
            echo "${container} exited before ${url} became ready."
            return 1
        fi

        sleep 1
    done

    echo "${url} did not become ready."
    return 1
}

trap cleanup EXIT

docker image inspect "${image}" >/dev/null
docker network create "${network}" >/dev/null

containers+=("${postgres}")
docker run --detach \
    --name "${postgres}" \
    --network "${network}" \
    --env POSTGRES_DB=postgres \
    --env POSTGRES_HOST_AUTH_METHOD=trust \
    --health-cmd "pg_isready --username postgres --dbname postgres" \
    --health-interval 1s \
    --health-timeout 5s \
    --health-retries 30 \
    postgres:18.4-alpine >/dev/null

containers+=("${dragonfly}")
docker run --detach \
    --name "${dragonfly}" \
    --network "${network}" \
    docker.dragonflydb.io/dragonflydb/dragonfly:v1.38.0 \
    --cache_mode=true \
    --maxmemory=256mb \
    --proactor_threads=1 \
    --primary_port_http_enabled=false >/dev/null

wait_for_container_health "${postgres}"
wait_for_container_health "${dragonfly}"

docker run --rm \
    --network "${network}" \
    --env RETSU_ENVIRONMENT=test \
    --env RETSU_LOGGING__FORMAT=json \
    --env "RETSU_DATABASE__URL=${database_url}" \
    --env "RETSU_CACHE__DISTRIBUTED__URL=${cache_url}" \
    "${image}" migrate

containers+=("${api}")
docker run --detach \
    --name "${api}" \
    --network "${network}" \
    --publish 127.0.0.1::2424 \
    --env RETSU_ENVIRONMENT=test \
    --env RETSU_HTTP__BIND_ADDRESS=0.0.0.0 \
    --env RETSU_LOGGING__FORMAT=json \
    --env "RETSU_DATABASE__URL=${database_url}" \
    --env "RETSU_CACHE__DISTRIBUTED__URL=${cache_url}" \
    "${image}" api >/dev/null

containers+=("${expired_worker}")
docker run --detach \
    --name "${expired_worker}" \
    --network "${network}" \
    --publish 127.0.0.1::24247 \
    --env RETSU_ENVIRONMENT=test \
    --env RETSU_WORKER__MANAGEMENT__BIND_ADDRESS=0.0.0.0 \
    --env RETSU_LOGGING__FORMAT=json \
    --env "RETSU_DATABASE__URL=${database_url}" \
    --env "RETSU_CACHE__DISTRIBUTED__URL=${cache_url}" \
    "${image}" worker run queue expired-message-cleaner >/dev/null

containers+=("${metrics_worker}")
docker run --detach \
    --name "${metrics_worker}" \
    --network "${network}" \
    --publish 127.0.0.1::24247 \
    --env RETSU_ENVIRONMENT=test \
    --env RETSU_WORKER__MANAGEMENT__BIND_ADDRESS=0.0.0.0 \
    --env RETSU_LOGGING__FORMAT=json \
    --env "RETSU_DATABASE__URL=${database_url}" \
    --env "RETSU_CACHE__DISTRIBUTED__URL=${cache_url}" \
    "${image}" worker run queue state-metrics-collector >/dev/null

api_port="$(published_port "${api}" 2424)"
expired_worker_port="$(published_port "${expired_worker}" 24247)"
metrics_worker_port="$(published_port "${metrics_worker}" 24247)"
base_url="http://127.0.0.1:${api_port}"

wait_for_http "${api}" "${base_url}/health/ready"
wait_for_http "${expired_worker}" "http://127.0.0.1:${expired_worker_port}/health/ready"
wait_for_http "${metrics_worker}" "http://127.0.0.1:${metrics_worker_port}/health/ready"

queue_name="container-runtime-${test_id}"
queue_response="$(
    curl --fail --silent --show-error \
        --connect-timeout 2 \
        --max-time 5 \
        --request POST \
        --header "content-type: application/json" \
        --data "{
            \"name\": \"${queue_name}\",
            \"visibility_timeout_seconds\": 30,
            \"max_delivery_attempts\": 3,
            \"default_message_ttl_seconds\": 300
        }" \
        "${base_url}/v1/queues"
)"
queue_id="$(jq --exit-status --raw-output '.id' <<<"${queue_response}")"

enqueue_response="$(
    curl --fail --silent --show-error \
        --connect-timeout 2 \
        --max-time 5 \
        --request POST \
        --header "content-type: application/json" \
        --data '{
            "payload": "container-runtime-integration",
            "priority": "HIGH"
        }' \
        "${base_url}/v1/queues/${queue_id}/messages"
)"
message_id="$(jq --exit-status --raw-output '.id' <<<"${enqueue_response}")"

dequeue_response="$(
    curl --fail --silent --show-error \
        --connect-timeout 2 \
        --max-time 5 \
        --request POST \
        "${base_url}/v1/queues/${queue_id}/messages/dequeue"
)"

jq --exit-status \
    --arg message_id "${message_id}" \
    '.id == $message_id
        and .payload == "container-runtime-integration"
        and .priority == "HIGH"
        and .delivery_attempts == 1
        and (.receipt_handle | type == "string")' \
    <<<"${dequeue_response}" >/dev/null

receipt_handle="$(jq --exit-status --raw-output '.receipt_handle' <<<"${dequeue_response}")"
acknowledge_status="$(
    curl --silent --show-error \
        --connect-timeout 2 \
        --max-time 5 \
        --output /dev/null \
        --write-out '%{http_code}' \
        --request POST \
        --header "content-type: application/json" \
        --data "{\"receipt_handle\":\"${receipt_handle}\"}" \
        "${base_url}/v1/queues/${queue_id}/messages/${message_id}/acknowledge"
)"

if [[ "${acknowledge_status}" != "204" ]]; then
    echo "Acknowledge returned HTTP ${acknowledge_status}, expected 204."
    exit 1
fi

empty_dequeue_status="$(
    curl --silent --show-error \
        --connect-timeout 2 \
        --max-time 5 \
        --output /dev/null \
        --write-out '%{http_code}' \
        --request POST \
        "${base_url}/v1/queues/${queue_id}/messages/dequeue"
)"

if [[ "${empty_dequeue_status}" != "204" ]]; then
    echo "Empty dequeue returned HTTP ${empty_dequeue_status}, expected 204."
    exit 1
fi

echo "Container image integration test passed."
