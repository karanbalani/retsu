#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

require_command() {
    local command_name="$1"

    if ! command -v "${command_name}" >/dev/null; then
        echo "${command_name} is required for local infrastructure validation." >&2
        return 1
    fi
}

for command_name in bash docker jq just node; do
    require_command "${command_name}"
done

just --fmt --check
bash -n scripts/local/*.sh scripts/release-tag.sh

compose=(
    docker compose
    --env-file .env.example
    --file infra/local/compose.yaml
)
"${compose[@]}" \
    --profile observability \
    --profile tracing \
    --profile load \
    config --quiet

dashboards=(infra/local/grafana/dashboards/*.json)
for dashboard in "${dashboards[@]}"; do
    jq --exit-status \
        '.uid | type == "string" and length > 0' \
        "${dashboard}" >/dev/null
done

duplicate_dashboard_uids="$(
    jq --raw-output '.uid' "${dashboards[@]}" | sort | uniq -d
)"
if [[ -n "${duplicate_dashboard_uids}" ]]; then
    echo "Grafana dashboard UIDs must be unique:" >&2
    echo "${duplicate_dashboard_uids}" >&2
    exit 1
fi

accounting_module_dir="$(mktemp -d)"
trap 'rm -rf "${accounting_module_dir}"' EXIT
cp load/k6/support/showcase.js \
    "${accounting_module_dir}/showcase.mjs"

SHOWCASE_MODULE_PATH="${accounting_module_dir}/showcase.mjs" \
    node --input-type=module <<'NODE'
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";

const showcase = await import(
    pathToFileURL(process.env.SHOWCASE_MODULE_PATH).href
);
const settings = {
    durationMinutes: 5,
    activeLoadMinutes: showcase.showcaseActiveLoadMinutes(5),
    headroom: 1.3,
    drainRate: 195,
    drainRampUpSeconds: 1,
    drainHoldSeconds: 29,
    drainRampDownSeconds: 2,
};
const expectedEnqueues = 17_460;
const stageIterations = (stages, startRate) => {
    let previousRate = startRate;
    let iterations = 0;

    for (const stage of stages) {
        const seconds = Number(stage.duration.slice(0, -1));
        iterations += ((previousRate + stage.target) * seconds) / 2;
        previousRate = stage.target;
    }

    return iterations;
};
const stageSeconds = (stages) =>
    stages.reduce(
        (total, stage) =>
            total + Number(stage.duration.slice(0, -1)),
        0,
    );

assert.equal(
    settings.activeLoadMinutes,
    3,
);
assert.equal(
    showcase.expectedShowcaseEnqueues(settings.activeLoadMinutes),
    expectedEnqueues,
);
assert.equal(
    stageIterations(
        showcase.showcaseProducerStages(settings.activeLoadMinutes),
        showcase.SHOWCASE_START_RATE,
    ),
    expectedEnqueues,
);
assert.equal(
    stageSeconds(
        showcase.showcaseProducerStages(settings.activeLoadMinutes),
    ),
    180,
);
assert.deepEqual(
    showcase.expectedShowcasePriorities(expectedEnqueues),
    { HIGH: 12_222, MEDIUM: 3_493, LOW: 1_745 },
);
assert.deepEqual(
    showcase.expectedShowcaseQueues(expectedEnqueues),
    {
        "hot-a": 6_111,
        "hot-b": 6_111,
        "warm-a": 1_746,
        "warm-b": 1_746,
        fault: 1_746,
    },
);
assert.deepEqual(
    showcase.expectedShowcaseCohorts(expectedEnqueues),
    {
        process_1s: 14_000,
        process_2s: 2_609,
        process_3s: 680,
        process_5s: 136,
        retry_once: 19,
        retry_twice: 8,
        dead_letter: 4,
        expiry: 4,
    },
);
assert.deepEqual(
    showcase.expectedShowcaseDeliveryAccounting(expectedEnqueues),
    {
        acknowledgements: {
            first: 17_425,
            second: 19,
            third: 8,
            total: 17_452,
        },
        attempts: {
            first: 17_460,
            second: 31,
            third: 12,
            total: 17_503,
        },
        deadLetters: 4,
        faultMessages: 35,
        intentionalNoAcks: 51,
        previouslyDeliveredExpirations: 4,
    },
);
assert.equal(
    showcase.expectedShowcaseMainConsumerIterations(
        settings.activeLoadMinutes,
        settings.headroom,
    ),
    22_698,
);
assert.equal(
    showcase.expectedShowcaseTailIterations(settings),
    5_980,
);
assert.equal(
    stageIterations(
        showcase.showcaseConsumerStages(settings),
        showcase.showcaseConsumerRate(
            showcase.SHOWCASE_START_RATE,
            settings.headroom,
        ),
    ),
    28_678,
);
assert.equal(
    stageSeconds(showcase.showcaseConsumerStages(settings)),
    212,
);
assert.equal(
    showcase.showcaseVerificationStartSeconds(settings),
    285,
);
assert.equal(
    showcase.showcaseVerificationStartSeconds(settings) +
        showcase.SHOWCASE_VERIFICATION_WINDOW_SECONDS,
    settings.durationMinutes * 60,
);
NODE

if ! docker info >/dev/null 2>&1; then
    echo "Docker must be running to validate Prometheus and k6." >&2
    exit 1
fi

prometheus_image="$(
    "${compose[@]}" --profile observability config --format json \
        | jq --raw-output '.services.prometheus.image'
)"
docker run --rm --network none \
    --volume "${repo_root}/infra/local/prometheus:/etc/prometheus:ro" \
    --entrypoint /bin/promtool \
    "${prometheus_image}" \
    check config /etc/prometheus/prometheus.yaml

k6_image="$(
    "${compose[@]}" --profile load config --format json \
        | jq --raw-output '.services.k6.image'
)"
for scenario in smoke enqueue consume mixed saturation showcase; do
    docker run --rm --network none \
        --volume "${repo_root}/load/k6:/scripts:ro" \
        "${k6_image}" inspect "/scripts/${scenario}.js" >/dev/null
done

echo "Local infrastructure validation passed."
